"""CLI entry point for hacienda-mcp."""

import sys

from .downloader import run_hacienda-mcp


def main():
    """Main entry point for the CLI."""
    args = sys.argv[1:]
    run_hacienda-mcp(args)


if __name__ == "__main__":
    main()
