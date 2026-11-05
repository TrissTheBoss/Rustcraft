package com.triss.rustcraft;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.minecraft.client.KeyMapping;
import com.mojang.blaze3d.platform.InputConstants;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.net.Socket;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.atomic.AtomicBoolean;

public final class RustcraftTriggerClient implements ClientModInitializer {
    private static final Logger LOGGER = LoggerFactory.getLogger("rustcraft_trigger");
    private static final String TOKEN = "local-only";
    private static final AtomicBoolean TRIGGER_IN_FLIGHT = new AtomicBoolean();
    private KeyMapping trigger;
    private boolean triggerWasDown;

    @Override public void onInitializeClient() {
        LOGGER.info("Rustcraft Trigger initialized: loopback=127.0.0.1:39281 key=J");
        trigger = KeyMappingHelper.registerKeyMapping(new KeyMapping(
            "key.rustcraft_trigger.launch", InputConstants.Type.KEYBOARD, 74,
            KeyMapping.Category.MISC));
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            boolean down = trigger.isDown();
            if (down && !triggerWasDown) sendTriggerAsync();
            while (trigger.consumeClick()) { /* drain Fabric's queue */ }
            triggerWasDown = down;
        });
    }

    private static void sendTriggerAsync() {
        if (!TRIGGER_IN_FLIGHT.compareAndSet(false, true)) {
            LOGGER.debug("Trigger ignored because another request is in flight");
            return;
        }
        LOGGER.info("Trigger queued");
        Thread.startVirtualThread(() -> {
            try {
                sendTrigger();
            } finally {
                TRIGGER_IN_FLIGHT.set(false);
            }
        });
    }

    private static void sendTrigger() {
        try (Socket socket = new Socket()) {
            socket.connect(new InetSocketAddress("127.0.0.1", 39281), 250);
            try (PrintWriter out = new PrintWriter(new OutputStreamWriter(socket.getOutputStream(), StandardCharsets.UTF_8), true);
                 BufferedReader in = new BufferedReader(new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8))) {
                out.println("RUSTCRAFT " + TOKEN);
                String response = in.readLine();
                LOGGER.info("Rustcraft trigger response: {}", response == null ? "NO_RESPONSE" : response);
            }
        } catch (Exception ignored) {
            LOGGER.warn("Rustcraft trigger unavailable; the game remains unaffected");
        }
    }
}
