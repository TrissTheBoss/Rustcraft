package com.triss.rustcraft;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

public final class RustcraftTriggerClient implements ClientModInitializer {
    private static final String TOKEN = "local-only";
    private KeyBinding trigger;

    @Override public void onInitializeClient() {
        trigger = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.rustcraft_trigger.launch", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_J,
            "category.rustcraft_trigger"));
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            while (trigger.wasPressed()) sendTrigger();
        });
    }

    private static void sendTrigger() {
        try (Socket socket = new Socket("127.0.0.1", 39281);
             PrintWriter out = new PrintWriter(new OutputStreamWriter(socket.getOutputStream(), StandardCharsets.UTF_8), true);
             BufferedReader in = new BufferedReader(new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8))) {
            out.println("RUSTCRAFT " + TOKEN);
            in.readLine();
        } catch (Exception ignored) {
            // Rustcraft may not be running; the game must remain unaffected.
        }
    }
}
