import { MantineProvider } from "@mantine/core";
import type { ReactElement, ReactNode } from "react";
import { darkTheme } from "../../theme";

export function AppMantineProvider({
	children,
}: {
	children: ReactNode;
}): ReactElement {
	return (
		<MantineProvider theme={darkTheme} defaultColorScheme="dark">
			{children}
		</MantineProvider>
	);
}
