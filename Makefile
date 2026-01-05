.PHONY: security-check

security-check:
	./scripts/utilities/asp-preflight.sh --staged --strict
	git secrets --scan --cached
