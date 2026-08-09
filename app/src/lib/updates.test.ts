import { describe, expect, it } from "vitest";
import {
	checkSignedUpdateVersion,
	compareSemver,
	installSignedUpdate,
	normalizeVersion,
	signedUpdaterEnabled,
} from "./updates";

describe("updates", () => {
	it("normalizes release tags and compares semantic versions", () => {
		expect(normalizeVersion(" v1.2.3-beta.1 ")).toBe("1.2.3");
		expect(normalizeVersion("not-a-version")).toBeNull();
		expect(compareSemver("2.0.0", "1.9.9")).toBe(1);
		expect(compareSemver("1.2.3", "1.2.3")).toBe(0);
		expect(compareSemver("1.2.2", "1.2.3")).toBe(-1);
	});

	it("keeps the signed updater disabled unless the release build opts in", async () => {
		expect(signedUpdaterEnabled).toBe(false);
		expect(await checkSignedUpdateVersion()).toBeNull();
		expect(await installSignedUpdate()).toBe(false);
	});
});
