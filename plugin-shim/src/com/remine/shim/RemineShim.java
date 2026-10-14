package com.remine.shim;

import com.google.gson.Gson;
import com.google.gson.reflect.TypeToken;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import org.bukkit.Bukkit;
import org.bukkit.Location;
import org.bukkit.entity.Entity;
import org.bukkit.entity.LivingEntity;
import org.bukkit.entity.Player;
import org.bukkit.plugin.java.JavaPlugin;
import org.bukkit.util.Vector;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

public class RemineShim extends JavaPlugin {
    private HttpServer http;
    private final Gson gson = new Gson();

    @Override
    public void onEnable() {
        saveDefaultConfig();
        int port = getConfig().getInt("port", 5124);
        try {
            http = HttpServer.create(new InetSocketAddress("127.0.0.1", port), 0);
            http.createContext("/health", this::health);
            http.createContext("/players", this::players);
            http.createContext("/entities", this::entities);
            http.createContext("/entity", this::entity);
            http.createContext("/exec", this::exec);
            http.setExecutor(Executors.newCachedThreadPool());
            http.start();
            getLogger().info("REMINE_SHIM shim http di 127.0.0.1:" + port);
        } catch (IOException e) {
            getLogger().severe("REMINE_SHIM gagal start http: " + e.getMessage());
        }
    }

    @Override
    public void onDisable() {
        if (http != null) {
            http.stop(0);
        }
    }

    private void reply(HttpExchange ex, int code, Object obj) throws IOException {
        byte[] body = gson.toJson(obj).getBytes(StandardCharsets.UTF_8);
        ex.getResponseHeaders().set("Content-Type", "application/json");
        ex.sendResponseHeaders(code, body.length);
        try (OutputStream os = ex.getResponseBody()) {
            os.write(body);
        }
    }

    private String query(HttpExchange ex, String key, String def) {
        String q = ex.getRequestURI().getRawQuery();
        if (q == null) {
            return def;
        }
        for (String part : q.split("&")) {
            int i = part.indexOf('=');
            if (i > 0 && part.substring(0, i).equals(key)) {
                return part.substring(i + 1);
            }
        }
        return def;
    }

    private Map<String, Object> sync(java.util.concurrent.Callable<Map<String, Object>> fn) {
        try {
            return Bukkit.getScheduler().callSyncMethod(this, fn).get(5, TimeUnit.SECONDS);
        } catch (Exception e) {
            Map<String, Object> err = new HashMap<>();
            err.put("ok", false);
            err.put("error", e.getMessage());
            return err;
        }
    }

    private Map<String, Object> playerMap(Player p) {
        Map<String, Object> m = new HashMap<>();
        Location l = p.getLocation();
        m.put("name", p.getName());
        m.put("uuid", p.getUniqueId().toString());
        m.put("world", l.getWorld().getName());
        m.put("x", l.getX());
        m.put("y", l.getY());
        m.put("z", l.getZ());
        m.put("health", p.getHealth());
        m.put("gamemode", p.getGameMode().name());
        return m;
    }

    private Map<String, Object> entityMap(Entity e) {
        Map<String, Object> m = new HashMap<>();
        Location l = e.getLocation();
        m.put("uuid", e.getUniqueId().toString());
        m.put("type", e.getType().name());
        m.put("name", e.getCustomName());
        if (e instanceof Player) {
            m.put("player", ((Player) e).getName());
        }
        m.put("world", l.getWorld().getName());
        m.put("x", l.getX());
        m.put("y", l.getY());
        m.put("z", l.getZ());
        m.put("yaw", l.getYaw());
        m.put("pitch", l.getPitch());
        Vector v = e.getVelocity();
        m.put("vx", v.getX());
        m.put("vy", v.getY());
        m.put("vz", v.getZ());
        m.put("glowing", e.isGlowing());
        if (e instanceof LivingEntity) {
            LivingEntity le = (LivingEntity) e;
            m.put("health", le.getHealth());
            m.put("max_health", le.getMaxHealth());
        }
        return m;
    }

    private void health(HttpExchange ex) throws IOException {
        Map<String, Object> m = new HashMap<>();
        m.put("ok", true);
        m.put("plugin", "RemineShim");
        m.put("version", getDescription().getVersion());
        reply(ex, 200, m);
    }

    private void players(HttpExchange ex) throws IOException {
        if (!ex.getRequestMethod().equals("GET")) {
            reply(ex, 405, Map.of("ok", false));
            return;
        }
        Map<String, Object> out = sync(() -> {
            List<Map<String, Object>> list = new ArrayList<>();
            for (Player p : Bukkit.getOnlinePlayers()) {
                list.add(playerMap(p));
            }
            Map<String, Object> m = new HashMap<>();
            m.put("ok", true);
            m.put("players", list);
            return m;
        });
        reply(ex, 200, out);
    }

    private void entities(HttpExchange ex) throws IOException {
        if (!ex.getRequestMethod().equals("GET")) {
            reply(ex, 405, Map.of("ok", false));
            return;
        }
        String worldName = query(ex, "world", "");
        int limit;
        try {
            limit = Integer.parseInt(query(ex, "limit", "100"));
        } catch (NumberFormatException nfe) {
            limit = 100;
        }
        int cap = Math.max(1, Math.min(limit, 1000));
        Map<String, Object> out = sync(() -> {
            List<Map<String, Object>> list = new ArrayList<>();
            for (org.bukkit.World w : Bukkit.getWorlds()) {
                if (!worldName.isEmpty() && !w.getName().equals(worldName)) {
                    continue;
                }
                for (Entity e : w.getEntities()) {
                    if (list.size() >= cap) {
                        break;
                    }
                    list.add(entityMap(e));
                }
            }
            Map<String, Object> m = new HashMap<>();
            m.put("ok", true);
            m.put("count", list.size());
            m.put("entities", list);
            return m;
        });
        reply(ex, 200, out);
    }

    private void entity(HttpExchange ex) throws IOException {
        String method = ex.getRequestMethod();
        if (method.equals("GET")) {
            String id = query(ex, "uuid", "");
            UUID uuid;
            try {
                uuid = UUID.fromString(id);
            } catch (IllegalArgumentException iae) {
                reply(ex, 400, Map.of("ok", false, "error", "uuid salah"));
                return;
            }
            Map<String, Object> out = sync(() -> {
                Entity e = Bukkit.getEntity(uuid);
                if (e == null) {
                    return Map.of("ok", false, "error", "entity tidak ada");
                }
                Map<String, Object> m = entityMap(e);
                m.put("ok", true);
                return m;
            });
            reply(ex, 200, out);
            return;
        }
        if (method.equals("POST")) {
            String body = new String(ex.getRequestBody().readAllBytes(), StandardCharsets.UTF_8);
            Map<String, Object> in = gson.fromJson(body, new TypeToken<Map<String, Object>>() {}.getType());
            if (in == null || !in.containsKey("uuid") || !in.containsKey("op")) {
                reply(ex, 400, Map.of("ok", false, "error", "butuh uuid dan op"));
                return;
            }
            UUID uuid;
            try {
                uuid = UUID.fromString(String.valueOf(in.get("uuid")));
            } catch (IllegalArgumentException iae) {
                reply(ex, 400, Map.of("ok", false, "error", "uuid salah"));
                return;
            }
            String op = String.valueOf(in.get("op"));
            Map<String, Object> out = sync(() -> {
                Entity e = Bukkit.getEntity(uuid);
                if (e == null) {
                    return Map.of("ok", false, "error", "entity tidak ada");
                }
                switch (op) {
                    case "tp": {
                        double x = num(in.get("x"));
                        double y = num(in.get("y"));
                        double z = num(in.get("z"));
                        boolean moved = e.teleport(new Location(e.getWorld(), x, y, z));
                        return Map.of("ok", moved);
                    }
                    case "remove": {
                        e.remove();
                        return Map.of("ok", true);
                    }
                    case "name": {
                        Object v = in.get("value");
                        e.setCustomName(v == null ? null : String.valueOf(v));
                        return Map.of("ok", true);
                    }
                    case "glow": {
                        Object v = in.get("value");
                        e.setGlowing(Boolean.parseBoolean(String.valueOf(v)));
                        return Map.of("ok", true);
                    }
                    case "velocity": {
                        e.setVelocity(new Vector(num(in.get("x")), num(in.get("y")), num(in.get("z"))));
                        return Map.of("ok", true);
                    }
                    default:
                        return Map.of("ok", false, "error", "op tidak dikenal");
                }
            });
            reply(ex, 200, out);
            return;
        }
        reply(ex, 405, Map.of("ok", false));
    }

    private void exec(HttpExchange ex) throws IOException {
        if (!ex.getRequestMethod().equals("POST")) {
            reply(ex, 405, Map.of("ok", false));
            return;
        }
        String body = new String(ex.getRequestBody().readAllBytes(), StandardCharsets.UTF_8);
        Map<String, Object> in = gson.fromJson(body, new TypeToken<Map<String, Object>>() {}.getType());
        if (in == null || !in.containsKey("command")) {
            reply(ex, 400, Map.of("ok", false, "error", "butuh command"));
            return;
        }
        String command = String.valueOf(in.get("command"));
        Map<String, Object> out = sync(() -> {
            boolean dispatched = Bukkit.dispatchCommand(Bukkit.getConsoleSender(), command);
            return Map.of("ok", dispatched, "command", command);
        });
        reply(ex, 200, out);
    }

    private static double num(Object o) {
        if (o instanceof Number) {
            return ((Number) o).doubleValue();
        }
        try {
            return Double.parseDouble(String.valueOf(o));
        } catch (NumberFormatException nfe) {
            return 0;
        }
    }
}
