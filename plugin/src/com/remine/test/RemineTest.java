package com.remine.test;

import org.bukkit.Bukkit;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.plugin.java.JavaPlugin;

public class RemineTest extends JavaPlugin implements Listener, CommandExecutor {
    @Override
    public void onEnable() {
        getServer().getPluginManager().registerEvents(this, this);
        getCommand("remine").setExecutor(this);
        getLogger().info("REMINE_ENABLE RemineTest aktif");
    }

    @Override
    public boolean onCommand(CommandSender sender, Command cmd, String label, String[] args) {
        String target = args.length > 0 ? String.join(" ", args) : "server";
        String who = sender instanceof Player ? ((Player) sender).getName() : "CONSOLE";
        sender.sendMessage("Halo " + target + ", dari RemineTest!");
        Bukkit.broadcastMessage(who + " menyapa lewat /remine");
        getLogger().info("REMINE_CMD sender=" + who + " arg=" + target);
        return true;
    }

    @EventHandler
    public void onJoin(PlayerJoinEvent e) {
        String name = e.getPlayer().getName();
        e.getPlayer().sendMessage("Selamat datang " + name + "! Ketik /remine");
        getLogger().info("REMINE_JOIN name=" + name);
    }
}
