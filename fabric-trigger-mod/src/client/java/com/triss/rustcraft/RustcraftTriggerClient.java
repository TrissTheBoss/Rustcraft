package com.triss.rustcraft;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.minecraft.client.KeyMapping;
import com.mojang.blaze3d.platform.InputConstants;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

public final class RustcraftTriggerClient implements ClientModInitializer {
    private static final String TOKEN = "local-only";
    private KeyMapping trigger;

    @Override public void onInitializeClient() {
        trigger = KeyMappingHelper.registerKeyMapping(new KeyMapping(
            "key.rustcraft_trigger.launch", InputConstants.Type.KEYBOARD, 74,
            KeyMapping.Category.MISC));
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            while (trigger.consumeClick()) sendTrigger();
        });
    }

    private static void sendTrigger() {
        try (Socket socket = new Socket("127.0.0.1", 39281);
             PrintWriter out = new PrintWriter(new OutputStreamWriter(socket.getOutputStream(), StandardCharsets.UTF_8), true);
             BufferedReader in = new BufferedReader(new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8))) {
            out.println("RUSTCRAFT " + TOKEN);
            in.readLine();
        } catch (Exception ignored) {
            // Rustcraft may not be running; the game remains unaffected.
        }
    }
}
