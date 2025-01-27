# Définition des cibles par défaut
TARGETS = x86_64-unknown-linux-gnu x86_64-pc-windows-gnu aarch64-unknown-linux-gnu
.DEFAULT_GOAL := local

# Détection automatique du système (Linux ou autre)
UNAME_S := $(shell uname -s)
IS_LINUX := $(filter Linux, $(UNAME_S))

.PHONY: all build test strip local

# Si aucune option, cible par défaut : build local
all: local

# Cible locale uniquement (par exemple, pour Linux x86_64 sur le système hôte)
local:
	cargo build --release
ifeq ($(IS_LINUX), Linux)
	@find target/release -maxdepth 1 -type f -exec strip {} \;
endif

# Build pour les cibles cross-compilées spécifiées dans TARGETS
build: $(TARGETS)

$(TARGETS):
	cross build --target $@ --release
ifeq ($(IS_LINUX), Linux)
	@find target/$@/release -maxdepth 1 -type f -exec strip {} \;
endif

# Exécution des tests pour chaque cible
test:
	@for target in $(TARGETS); do \
		cross test --target $$target --release; \
	done
