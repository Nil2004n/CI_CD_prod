# PWT (Phase 4 rebuild) — entry points.
#
# Variables (all overridable on the command line):
#   JAVA_HOME   JDK home; $JAVA_HOME/bin/java is preferred, else PATH java.
#   DATA_DIR    Shared application data directory (GUI, CLI, MCP).
#               Default: ~/.pwt-data
#   ROOT        Absolute project directory for repo-add / setup-* targets.
#   ALLOW_SCANS Empty = preserve an existing PWT permission mode (fresh
#               setups default to read-only). 1 = scan-enabled, 0 = read-only.
#
# Side effects per target:
#   help, doctor, repo-list, mcp-status: read-only (doctor never builds).
#   build, test: compile/test only.
#   setup, repo-add: create the data dir / register a root (idempotent).
#   start, dashboard, mcp: launch processes (dashboard/GUI need a display).
#   setup-opencode, setup-claude: write project-local client files only;
#     never global client configuration; preserve unrelated settings.
#
# Examples:
#   make help
#   make doctor
#   make setup
#   make repo-add ROOT="/absolute/project"
#   make scan ROOT=/tmp/repo FILES="a.yaml"          # headless scan
#   make setup-opencode ROOT="/absolute/project" ALLOW_SCANS=1
#   make setup-claude ROOT="/absolute/project" ALLOW_SCANS=0
#   make mcp                       # read-only MCP stdio server
#   make mcp ALLOW_SCANS=1         # scan-enabled MCP stdio server

JAVA_HOME ?= /usr/lib/jvm/java-21-openjdk-amd64
export JAVA_HOME
export PATH := $(JAVA_HOME)/bin:$(PATH)

DATA_DIR ?= $(HOME)/.pwt-data
ROOT ?= $(CURDIR)
ALLOW_SCANS ?=

APP_VERSION := $(shell grep -m1 '<version>' pwt/pom.xml | sed -e 's/.*<version>//' -e 's/<\/version.*//')
JAR := pwt/target/pwt-$(APP_VERSION).jar
ABS_JAR := $(CURDIR)/$(JAR)
LAUNCHER := $(CURDIR)/integrations/pwt-mcp-stdio.sh
DASHBOARD := $(CURDIR)/integrations/pwt-dashboard.sh

ALLOW_FLAG := $(if $(filter 1,$(ALLOW_SCANS)),--allow-scans)
MODE_DESC := $(if $(filter 1,$(ALLOW_SCANS)),scan-enabled,$(if $(filter 0,$(ALLOW_SCANS)),read-only,preserve-existing-or-read-only))

.PHONY: help doctor build test setup repo-add repo-list start dashboard \
	setup-opencode setup-claude mcp mcp-status clean

help:
	@echo "PWT targets (variables: JAVA_HOME DATA_DIR ROOT ALLOW_SCANS):"
	@echo "  make help            this message"
	@echo "  make doctor          Java/artifact/data-dir/repos/scanner report (read-only)"
	@echo "  make build           build $(JAR) (skips tests)"
	@echo "  make test            full JUnit suite"
	@echo "  make setup           create DATA_DIR, build if needed, print prerequisites"
	@echo "  make repo-add ROOT=<dir>   register a repository root (idempotent, shared store)"
	@echo "  make repo-list       show registered roots and availability"
	@echo "  make start           launch the desktop GUI on DATA_DIR"
	@echo "  make dashboard       open the GUI via the dashboard helper (local display only)"
	@echo "  make setup-opencode ROOT=<dir> [ALLOW_SCANS=1|0]   project-local OpenCode config"
	@echo "  make setup-claude ROOT=<dir> [ALLOW_SCANS=1|0]     project-local Claude Code config"
	@echo "  make mcp [ALLOW_SCANS=1]   MCP stdio server (stdout is protocol-only)"
	@echo "  make mcp-status      recorded MCP session presence and permission modes"
	@echo "Permission modes: read-only (default) vs scan-enabled (--allow-scans)."

build: $(JAR)

$(JAR): $(shell find pwt/src pwt/pom.xml -type f 2>/dev/null) pwt/mvnw
	cd pwt && ./mvnw -B -q package -DskipTests

test:
	cd pwt && ./mvnw -B test

setup:
	@mkdir -p "$(DATA_DIR)"
	@if [ ! -f "$(JAR)" ]; then $(MAKE) build; fi
	@echo "setup: data directory ready: $(DATA_DIR)"
	@echo "setup: Java: $$(command -v java) ($$($(JAVA_HOME)/bin/java -version 2>&1 | head -1))"
	@for tool in yamllint gitleaks checkov actionlint kubeconform; do \
		if command -v "$$tool" >/dev/null 2>&1; then \
			echo "setup: scanner present: $$tool"; \
		else \
			echo "setup: scanner MISSING: $$tool (install it to enable that adapter)"; \
		fi; \
	done
	@echo "setup: missing scanners surface as UNAVAILABLE, never as clean."

doctor:
	@if [ ! -f "$(JAR)" ]; then \
		echo "doctor: application jar missing: $(JAR)" >&2; \
		echo "doctor: run 'make build' first (doctor never builds)" >&2; \
		exit 2; \
	fi
	@java -jar "$(JAR)" doctor --data-dir "$(DATA_DIR)"

repo-add:
ifndef ROOT
	$(error Usage: make repo-add ROOT=<directory> [DATA_DIR=<dir>])
endif
	@if [ ! -f "$(JAR)" ]; then $(MAKE) build; fi
	@java -jar "$(JAR)" repo-add --root "$(ROOT)" --data-dir "$(DATA_DIR)"

repo-list:
	@if [ ! -f "$(JAR)" ]; then $(MAKE) build; fi
	@java -jar "$(JAR)" repo-list --data-dir "$(DATA_DIR)"

start:
	@if [ ! -f "$(JAR)" ]; then $(MAKE) build; fi
	java -jar "$(JAR)" gui --data-dir "$(DATA_DIR)"

dashboard:
	@if [ ! -f "$(JAR)" ]; then $(MAKE) build; fi
	"$(DASHBOARD)" --jar "$(ABS_JAR)" --data-dir "$(DATA_DIR)"

setup-opencode:
	@command -v python3 >/dev/null 2>&1 || { echo "setup-opencode: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/setup_opencode.py" --project-dir "$(ROOT)" \
		--data-dir "$(DATA_DIR)" $(if $(ALLOW_SCANS),--allow-scans $(ALLOW_SCANS))
	@echo "setup-opencode: mode handling: $(MODE_DESC)"

setup-claude:
	@command -v python3 >/dev/null 2>&1 || { echo "setup-claude: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/setup_claude.py" --project-dir "$(ROOT)" \
		--data-dir "$(DATA_DIR)" $(if $(ALLOW_SCANS),--allow-scans $(ALLOW_SCANS))
	@echo "setup-claude: mode handling: $(MODE_DESC)"

mcp:
	@"$(LAUNCHER)" --data-dir "$(DATA_DIR)" $(ALLOW_FLAG)

mcp-status:
	@if [ ! -f "$(JAR)" ]; then $(MAKE) build; fi
	@java -jar "$(JAR)" mcp-status --data-dir "$(DATA_DIR)"

clean:
	cd pwt && ./mvnw -B -q clean
