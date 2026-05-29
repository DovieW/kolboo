import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { AccountActionsCard } from "./AccountActionsCard";

describe("AccountActionsCard", () => {
	it("treats signed-out self-serve users as valid Community/BYOK candidates", () => {
		const html = renderToStaticMarkup(
			<MantineProvider>
				<AccountActionsCard
					signedIn={false}
					reauthRequired={false}
					loginPending={false}
					signupPending={false}
					refreshPending={false}
					logoutPending={false}
					managePending={false}
					manageAvailable={false}
					onPasswordSignIn={vi.fn()}
					onPasswordSignUp={vi.fn()}
					onBrowserSignIn={vi.fn()}
					onRefresh={vi.fn()}
					onManage={vi.fn()}
					onSignOut={vi.fn()}
				/>
			</MantineProvider>,
		);

		expect(html).toContain("Sign in to Kolboo");
		expect(html).toContain("Create free account");
		expect(html).toContain("Community/BYOK session now");
		expect(html).toContain("settings sync and managed inference");
		expect(html).toContain("confirm the email first");
		expect(html).toContain(
			"rechecks any ready organization or paid-access claims",
		);
	});
});
