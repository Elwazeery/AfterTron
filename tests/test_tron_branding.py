"""Source-level Tron branding, security, GemiTokens, GemiMCP, and GemiGuard contract tests."""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "apps" / "effectcraft"
BRAND = "AfterTron"
UPSTREAM = "EffectCraft"


class BrandingContract(unittest.TestCase):
    def test_full_upstream_source_present(self):
        for path in ("Cargo.toml", "Cargo.lock", "LICENSE-MIT", "LICENSE-APACHE"):
            with self.subTest(path=path):
                self.assertTrue((ROOT / path).is_file(), f"Missing original fork source/license: {path}")
        self.assertTrue((APP / "Cargo.toml").is_file())

    def test_native_window_and_windows_metadata(self):
        main = (APP / "src" / "main.rs").read_text(encoding="utf-8")
        build = (APP / "build.rs").read_text(encoding="utf-8")
        self.assertIn(f'"{BRAND}"', main, "Native window identity must show Tron brand")
        self.assertNotIn(f'.with_title("{UPSTREAM}")', main)
        self.assertNotIn(f'run_native(\n        "{UPSTREAM}"', main)
        self.assertIn("--control-token-out", main, "CLI must support owner-only control token output file")
        self.assertIn(f'.set("ProductName", "{BRAND}")', build)
        self.assertIn(f'.set("FileDescription", "{BRAND} ', build)
        self.assertIn("LegalCopyright", build, "Fork must retain original legal attribution")
        self.assertIn('OriginalFilename", "effectcraft.exe"', build,
                      "Native executable/installer ID migration requires compatibility tests")

    def test_inferno_pixel_glass_and_gemifont_integrated(self):
        theme_rs = (ROOT / "crates" / "ui-egui" / "src" / "theme.rs").read_text(encoding="utf-8")
        self.assertIn("InfernoDark", theme_rs)
        self.assertIn("InfernoLight", theme_rs)
        self.assertIn("inferno-dark", theme_rs)
        self.assertIn("inferno-light", theme_rs)
        for weight in ("Light", "Medium", "Bold"):
            font_file = ROOT / "assets" / "fonts" / f"GemiFontSans-{weight}.ttf"
            sidecar = ROOT / "assets" / "fonts" / f"GemiFontSans-{weight}.ttf.attribution"
            self.assertTrue(font_file.is_file(), f"Missing GemiFontSans-{weight}.ttf")
            self.assertTrue(sidecar.is_file(), f"Missing sidecar for GemiFontSans-{weight}.ttf")

    def test_gemimcp_manifest_and_security_contracts(self):
        manifest_path = ROOT / "contracts" / "gemimcp-product-aftertron.v1.json"
        self.assertTrue(manifest_path.is_file(), "Missing GemiMCP product manifest JSON")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(manifest["schema"], "gemimcp.product-manifest.v1")
        self.assertEqual(manifest["productId"], "aftertron")
        self.assertEqual(manifest["namespace"], "after")
        self.assertEqual(manifest["upstreamBase"], "storytold/effectcraft")
        self.assertEqual(manifest["auth"]["issuer"], "https://auth.gemitron.net")
        self.assertGreaterEqual(manifest["capabilities"]["totalToolCount"], 750)

        control_srv = (APP / "src" / "control_server.rs").read_text(encoding="utf-8")
        self.assertIn("ConnectionSlot", control_srv)
        self.assertIn("AUTH_DEADLINE", control_srv)
        self.assertIn("validate_loopback_bind", control_srv)

        for shot in (
            "aftertron-inferno-dark-hero.png",
            "aftertron-graph-editor-effects.png",
            "aftertron-render-queue-and-scopes.png",
            "aftertron-inferno-light-workspace.png",
            "aftertron-about-and-settings.png",
        ):
            self.assertTrue((ROOT / "docs" / "images" / "aftertron" / shot).is_file(), f"Missing screenshot {shot}")
            self.assertTrue((ROOT / "docs" / "images" / "aftertron" / f"{shot}.attribution").is_file(), f"Missing sidecar for {shot}")

    def test_product_contract_still_exists(self):
        self.assertTrue((ROOT / "TODO.md").is_file())
        self.assertTrue((ROOT / "docs" / "01-FORK-PRODUCT-AND-CLOUD-MCP-CHARTER.md").is_file())
        self.assertTrue((ROOT / "docs" / "implementation" / "07-NATIVE-CONTROL-AND-MCP-SECURITY-AUDIT.md").is_file())
        self.assertTrue((ROOT / "docs" / "implementation" / "08-SOURCE-COMPLETENESS-AND-PARITY-MATRIX.md").is_file())
        self.assertTrue((ROOT / "scripts" / "import-upstream.sh").is_file())


if __name__ == "__main__":
    unittest.main()
