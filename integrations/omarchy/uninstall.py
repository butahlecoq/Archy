#!/usr/bin/env python3
"""Preview or apply removal of the unchanged Archy user plugin."""
from install import main
import sys

if __name__ == "__main__":
    sys.argv.insert(1, "--uninstall")
    sys.exit(main())
