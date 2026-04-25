

install-npmn:
	@echo "Installed pnpm"
	npm install -g pnpm


install-dependencies: install-npmn
	@echo "Installed node dependencies"	
	pnpm install
	@echo "Installed rust dependencies"
	cargo install tauri-cli --version "^2"


gen-icons:
	@echo "Generated icons"
	pnpm tauri icon resources/images/work.png -o src-tauri/icons

