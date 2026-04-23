

install-npmn:
	@echo "Installed pnpm"
	npm install -g pnpm


install-dependencies: install-npmn
	@echo "Installed node dependencies"	
	pnpm install
	@echo "Installed rust dependencies"
	cargo install tauri-cli --version "^2"

