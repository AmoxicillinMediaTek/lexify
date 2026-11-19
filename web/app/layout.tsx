import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Lexify Studio | Lua obfuscator",
  description: "A local-first demo for obfuscating Lua source with Lexify.",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
