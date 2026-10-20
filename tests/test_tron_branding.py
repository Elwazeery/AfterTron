"""Source-level Tron branding guard. Real Windows/macOS builds remain separate gates."""
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
        self.assertIn(f'.set("ProductName", "{BRAND}")', build)
        self.assertIn(f'.set("FileDescription", "{BRAND} ', build)
        self.assertIn("LegalCopyright", build, "Fork must retain original legal attribution")
        self.assertIn('OriginalFilename", "effectcraft.exe"', build,
                      "Native executable/installer ID migration requires compatibility tests")

    def test_product_contract_still_exists(self):
        self.assertTrue((ROOT / "TODO.md").is_file())
        self.assertTrue((ROOT / "docs" / "01-FORK-PRODUCT-AND-CLOUD-MCP-CHARTER.md").is_file())
        self.assertTrue((ROOT / "scripts" / "import-upstream.sh").is_file())


if __name__ == "__main__":
    unittest.main()
