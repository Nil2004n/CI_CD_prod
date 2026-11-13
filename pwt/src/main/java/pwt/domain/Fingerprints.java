package pwt.domain;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.HexFormat;

/**
 * Fingerprint version 1: SHA-256 over normalized adapter ID, rule ID,
 * relative path, and non-sensitive location fallback. Never incorporates
 * secret values or raw evidence.
 */
public final class Fingerprints {

    private Fingerprints() {
    }

    public static String v1(String adapterId, String ruleId, String relativePath,
            Integer lineStart, Integer columnStart) {
        String normalized = String.join("\0", "pwt-fingerprint-v1",
                adapterId == null ? "" : adapterId,
                ruleId == null ? "" : ruleId,
                relativePath == null ? "" : relativePath,
                lineStart == null ? "" : lineStart.toString(),
                columnStart == null ? "" : columnStart.toString());
        try {
            MessageDigest sha256 = MessageDigest.getInstance("SHA-256");
            return HexFormat.of().formatHex(sha256.digest(normalized.getBytes(StandardCharsets.UTF_8)));
        } catch (java.security.NoSuchAlgorithmException impossible) {
            throw new IllegalStateException("SHA-256 unavailable", impossible);
        }
    }
}
