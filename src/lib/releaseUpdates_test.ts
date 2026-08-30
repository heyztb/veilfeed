import { assertEquals } from "@std/assert";
import {
  automaticCheckIntervalMs,
  automaticUpdateCheckDue,
  automaticUpdateChecksEnabled,
  dismissedReleaseVersion,
  dismissRelease,
  markUpdateCheckAttempt,
  setAutomaticUpdateChecksEnabled,
  shouldShowRelease,
} from "./releaseUpdates.ts";

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string) {
    return this.values.get(key) ?? null;
  }
  setItem(key: string, value: string) {
    this.values.set(key, value);
  }
}

Deno.test("automatic release checks are opt-in and limited to once daily", () => {
  const storage = new MemoryStorage();
  const now = Date.UTC(2026, 7, 31);
  assertEquals(automaticUpdateChecksEnabled(storage), false);
  assertEquals(automaticUpdateCheckDue(now, storage), false);
  setAutomaticUpdateChecksEnabled(true, storage);
  assertEquals(automaticUpdateCheckDue(now, storage), true);
  markUpdateCheckAttempt(now, storage);
  assertEquals(
    automaticUpdateCheckDue(now + automaticCheckIntervalMs - 1, storage),
    false,
  );
  assertEquals(
    automaticUpdateCheckDue(now + automaticCheckIntervalMs, storage),
    true,
  );
});

Deno.test("manual attempts can be recorded without changing the preference", () => {
  const storage = new MemoryStorage();
  markUpdateCheckAttempt(1234, storage);
  assertEquals(automaticUpdateChecksEnabled(storage), false);
  assertEquals(
    automaticUpdateCheckDue(1234 + automaticCheckIntervalMs, storage),
    false,
  );
});

Deno.test("dismissal is scoped to one release version", () => {
  const storage = new MemoryStorage();
  const result = {
    currentVersion: "1.0.0",
    available: {
      version: "1.1.0",
      title: "Veilfeed 1.1.0",
      releaseUrl: "https://github.com/acme/veilfeed/releases/tag/v1.1.0",
      publishedAt: "2026-08-31T12:00:00Z",
    },
  };
  assertEquals(
    shouldShowRelease(result, dismissedReleaseVersion(storage)),
    true,
  );
  dismissRelease("1.1.0", storage);
  assertEquals(
    shouldShowRelease(result, dismissedReleaseVersion(storage)),
    false,
  );
  assertEquals(
    shouldShowRelease(
      { ...result, available: { ...result.available, version: "1.2.0" } },
      dismissedReleaseVersion(storage),
    ),
    true,
  );
});
