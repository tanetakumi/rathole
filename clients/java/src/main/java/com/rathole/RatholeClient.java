package com.rathole;

import com.google.gson.*;
import java.io.*;
import java.net.*;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.concurrent.*;

/**
 * Rathole Java Client
 *
 * Connects to a Rathole server (written in Rust) and establishes a tunnel
 * to expose a local port through the server.
 *
 * Protocol: JSON messages with [length:u32_le][json_data] format
 */
public class RatholeClient {
    private final String serverAddr;
    private final int serverPort;
    private final int localPort;

    private Socket controlSocket;
    private DataInputStream controlInput;
    private DataOutputStream controlOutput;
    private final Gson gson;

    private int assignedPort = -1;
    private volatile boolean running = false;
    private final ExecutorService executorService;

    private static final int HEARTBEAT_INTERVAL_MS = 20000; // 20 seconds

    public RatholeClient(String serverAddr, int serverPort, int localPort) {
        this.serverAddr = serverAddr;
        this.serverPort = serverPort;
        this.localPort = localPort;
        this.gson = new Gson();
        this.executorService = Executors.newCachedThreadPool();
    }

    /**
     * Start the tunnel
     */
    public void start() throws IOException {
        System.out.println("Connecting to " + serverAddr + ":" + serverPort + "...");

        // Connect to server
        controlSocket = new Socket(serverAddr, serverPort);
        controlInput = new DataInputStream(controlSocket.getInputStream());
        controlOutput = new DataOutputStream(controlSocket.getOutputStream());

        // Send TunnelRequest
        Message tunnelRequest = new TunnelRequest(localPort);
        sendMessage(tunnelRequest);
        System.out.println("Sent TunnelRequest for local port " + localPort);

        // Receive TunnelResponse
        Message response = receiveMessage();
        if (response instanceof TunnelResponse) {
            assignedPort = ((TunnelResponse) response).assignedPort;
            System.out.println("✅ Tunnel established! Remote port: " + assignedPort);
            System.out.println("Access your service at: " + serverAddr + ":" + assignedPort);
        } else {
            throw new IOException("Expected TunnelResponse, got: " + response.getClass().getSimpleName());
        }

        running = true;

        // Start control channel handler
        executorService.submit(this::handleControlChannel);

        // Start heartbeat sender
        executorService.submit(this::sendHeartbeats);
    }

    /**
     * Handle control channel messages
     */
    private void handleControlChannel() {
        try {
            while (running) {
                Message msg = receiveMessage();

                if (msg instanceof CreateDataChannel) {
                    System.out.println("📡 Server requested data channel");
                    executorService.submit(this::createDataChannel);
                } else if (msg instanceof Heartbeat) {
                    System.out.println("💓 Received heartbeat");
                    sendMessage(new Heartbeat());
                }
            }
        } catch (IOException e) {
            if (running) {
                System.err.println("Control channel error: " + e.getMessage());
            }
        } finally {
            stop();
        }
    }

    /**
     * Send heartbeats periodically
     */
    private void sendHeartbeats() {
        try {
            while (running) {
                Thread.sleep(HEARTBEAT_INTERVAL_MS);
                if (running) {
                    sendMessage(new Heartbeat());
                    System.out.println("💓 Sent heartbeat");
                }
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        } catch (IOException e) {
            System.err.println("Failed to send heartbeat: " + e.getMessage());
        }
    }

    /**
     * Create a data channel for forwarding traffic
     */
    private void createDataChannel() {
        Socket serverSocket = null;
        Socket localSocket = null;

        try {
            // Connect to server
            serverSocket = new Socket(serverAddr, serverPort);
            System.out.println("✅ Data channel connected to server");

            // Connect to local service
            localSocket = new Socket("127.0.0.1", localPort);
            System.out.println("✅ Data channel connected to local service at port " + localPort);

            // Bidirectional copy
            Socket finalServerSocket = serverSocket;
            Socket finalLocalSocket = localSocket;

            // Server -> Local
            Thread serverToLocal = new Thread(() -> {
                try {
                    copyStream(finalServerSocket.getInputStream(), finalLocalSocket.getOutputStream());
                } catch (IOException e) {
                    System.err.println("Server -> Local error: " + e.getMessage());
                }
            });

            // Local -> Server
            Thread localToServer = new Thread(() -> {
                try {
                    copyStream(finalLocalSocket.getInputStream(), finalServerSocket.getOutputStream());
                } catch (IOException e) {
                    System.err.println("Local -> Server error: " + e.getMessage());
                }
            });

            serverToLocal.start();
            localToServer.start();

            serverToLocal.join();
            localToServer.join();

            System.out.println("Data channel closed");

        } catch (IOException | InterruptedException e) {
            System.err.println("Data channel error: " + e.getMessage());
        } finally {
            closeQuietly(serverSocket);
            closeQuietly(localSocket);
        }
    }

    /**
     * Copy data from input to output stream
     */
    private void copyStream(InputStream input, OutputStream output) throws IOException {
        byte[] buffer = new byte[8192];
        int bytesRead;
        while ((bytesRead = input.read(buffer)) != -1) {
            output.write(buffer, 0, bytesRead);
            output.flush();
        }
    }

    /**
     * Send a message to the server
     */
    private synchronized void sendMessage(Message msg) throws IOException {
        String json = gson.toJson(msg);
        byte[] data = json.getBytes("UTF-8");

        // Write length (u32, little-endian)
        ByteBuffer lengthBuffer = ByteBuffer.allocate(4);
        lengthBuffer.order(ByteOrder.LITTLE_ENDIAN);
        lengthBuffer.putInt(data.length);
        controlOutput.write(lengthBuffer.array());

        // Write JSON data
        controlOutput.write(data);
        controlOutput.flush();
    }

    /**
     * Receive a message from the server
     */
    private Message receiveMessage() throws IOException {
        // Read length (u32, little-endian)
        byte[] lengthBytes = new byte[4];
        controlInput.readFully(lengthBytes);
        ByteBuffer lengthBuffer = ByteBuffer.wrap(lengthBytes);
        lengthBuffer.order(ByteOrder.LITTLE_ENDIAN);
        int length = lengthBuffer.getInt();

        // Read JSON data
        byte[] data = new byte[length];
        controlInput.readFully(data);

        String json = new String(data, "UTF-8");

        // Parse JSON
        JsonObject jsonObject = JsonParser.parseString(json).getAsJsonObject();
        String type = jsonObject.get("type").getAsString();

        switch (type) {
            case "TunnelResponse":
                int assignedPort = jsonObject.get("assigned_port").getAsInt();
                return new TunnelResponse(assignedPort);
            case "CreateDataChannel":
                return new CreateDataChannel();
            case "Heartbeat":
                return new Heartbeat();
            default:
                throw new IOException("Unknown message type: " + type);
        }
    }

    /**
     * Stop the client
     */
    public void stop() {
        running = false;
        executorService.shutdownNow();
        closeQuietly(controlSocket);
        System.out.println("Client stopped");
    }

    /**
     * Get the assigned remote port
     */
    public int getAssignedPort() {
        return assignedPort;
    }

    private void closeQuietly(Socket socket) {
        if (socket != null) {
            try {
                socket.close();
            } catch (IOException ignored) {
            }
        }
    }

    // Message classes
    static abstract class Message {
        String type;
    }

    static class TunnelRequest extends Message {
        int local_port;

        TunnelRequest(int localPort) {
            this.type = "TunnelRequest";
            this.local_port = localPort;
        }
    }

    static class TunnelResponse extends Message {
        int assigned_port;
    }

    static class CreateDataChannel extends Message {
        CreateDataChannel() {
            this.type = "CreateDataChannel";
        }
    }

    static class Heartbeat extends Message {
        Heartbeat() {
            this.type = "Heartbeat";
        }
    }

    /**
     * Main method for testing
     */
    public static void main(String[] args) {
        if (args.length != 3) {
            System.out.println("Usage: java RatholeClient <server_addr> <server_port> <local_port>");
            System.out.println("Example: java RatholeClient localhost 2333 8080");
            System.exit(1);
        }

        String serverAddr = args[0];
        int serverPort = Integer.parseInt(args[1]);
        int localPort = Integer.parseInt(args[2]);

        RatholeClient client = new RatholeClient(serverAddr, serverPort, localPort);

        try {
            client.start();

            // Wait for Ctrl+C
            Runtime.getRuntime().addShutdownHook(new Thread(() -> {
                System.out.println("\nShutting down...");
                client.stop();
            }));

            System.out.println("Press Ctrl+C to stop...");
            Thread.currentThread().join();

        } catch (Exception e) {
            System.err.println("Error: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);
        }
    }
}
