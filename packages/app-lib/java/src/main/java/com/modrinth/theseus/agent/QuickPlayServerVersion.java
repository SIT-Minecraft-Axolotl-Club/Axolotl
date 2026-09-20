package com.modrinth.theseus.agent;

// Must be kept up-to-date with quick_play_version.rs
//
// Declared oldest-first so both sides share the same chronological ordering.
public enum QuickPlayServerVersion {
    UNSUPPORTED,
    INJECTED,
    BUILTIN_LEGACY,
    BUILTIN;

    public static final QuickPlayServerVersion CURRENT =
            valueOf(System.getProperty("modrinth.internal.quickPlay.serverVersion"));
}
