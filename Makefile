# PWT (Phase 4 rebuild) — entry points.
#
# Variables (all overridable on the command line):
#   JAVA_HOME   JDK home; $JAVA_HOME/bin/java is preferred, else PATH java.
#   DATA_DIR    Shared application data directory (GUI, CLI, MCP): ONE database
#               for every repository and scan.
#               Default: <application-root>/.pwt-data (this Makefile's directory).
#   ROOT        Absolute project directory for repo-add / setup-* targets.
#   ALLOW_SCANS Ignored by OpenCode targets (setup-opencode/setup-opencode-global/
#               enable): OpenCode entries are always dashboard-only (never
#               --allow-scans). setup-claude refuses ALLOW_SCANS=1 (Claude Code
#               is dashboard-only too). Still honored by `make mcp` for
#               deliberate local-admin uses.
#   ALLOW_ANALYSIS
#             Empty = preserve existing mode (fresh default: denied).
#             1 = allow agent analysis writes, 0 = deny.
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
#   make setup-opencode ROOT="/absolute/project" [ALLOW_ANALYSIS=1]
#   make setup-claude ROOT="/absolute/project" ALLOW_SCANS=0
#   make mcp                       # read-only MCP stdio server
#   make mcp ALLOW_SCANS=1         # scan-enabled MCP stdio server

JAVA_HOME ?= /usr/lib/jvm/java-21-openjdk-amd64
export JAVA_HOME
export PATH := $(JAVA_HOME)/bin:$(PATH)

DATA_DIR ?= $(CURDIR)/.pwt-data
ROOT ?= $(CURDIR)
ALLOW_SCANS ?=
ALLOW_ANALYSIS ?=
PREFIX ?= $(HOME)/.local/bin
APPSDIR ?= $(HOME)/.local/share/applications
DESKTOP_FILE := $(APPSDIR)/pwt-yaml-workbench.desktop

APP_VERSION := $(shell grep -m1 '<version>' pwt/pom.xml | sed -e 's/.*<version>//' -e 's/<\/version.*//')
JAR := pwt/target/pwt-$(APP_VERSION).jar
ABS_JAR := $(CURDIR)/$(JAR)
LAUNCHER := $(CURDIR)/integrations/pwt-mcp-stdio.sh
DASHBOARD := $(CURDIR)/integrations/pwt-dashboard.sh

ALLOW_FLAG := $(if $(filter 1,$(ALLOW_SCANS)),--allow-scans)
ANALYSIS_FLAG := $(if $(filter 1,$(ALLOW_ANALYSIS)),--allow-analysis-writes)
MODE_DESC := $(if $(filter 1,$(ALLOW_SCANS)),scan-enabled,$(if $(filter 0,$(ALLOW_SCANS)),read-only,preserve-existing-or-read-only))$(if $(filter 1,$(ALLOW_ANALYSIS)),+analysis-writes,)

.PHONY: help doctor build test setup repo-add repo-list start dashboard \
	setup-opencode setup-claude install-opencode uninstall-opencode mcp mcp-status clean \
	install-claude uninstall-claude claude-status test-integrations setup-claude-global \
	install uninstall enable purge-data install-shell-rc setup-opencode-global

help:
	@echo "PWT targets (variables: JAVA_HOME DATA_DIR ROOT ALLOW_ANALYSIS):"
	@echo "  make help            this message"
	@echo "  make doctor          Java/artifact/data-dir/repos/scanner report (read-only)"
	@echo "  make build           build $(JAR) (skips tests)"
	@echo "  make test            full JUnit suite"
	@echo "  make test-integrations   installer/integration tests (Python + Node; real CLI + MCP server)"
	@echo "  make setup           create DATA_DIR, build if needed, print prerequisites"
	@echo "  make repo-add ROOT=<dir>   register a repository root (idempotent, shared store)"
	@echo "  make repo-list       show registered roots and availability"
	@echo "  make start           launch the desktop GUI on DATA_DIR"
	@echo "  make dashboard       open the GUI via the dashboard helper (local display only)"
	@echo "  make setup-opencode ROOT=<dir> [ALLOW_ANALYSIS=1|0]"
	@echo "      project-local OpenCode config (native /pwt commands + read-only MCP entry)"
	@echo "  make setup-opencode-global [ALLOW_ANALYSIS=1|0]"
	@echo "      user-global pwt MCP entry (any directory; unscoped, read-only default)"
	@echo "  make install-claude  user-wide Claude Code /pwt, /pwt-ask and pwt subagent (managed files)"
	@echo "  make setup-claude ROOT=<dir> [ALLOW_ANALYSIS=1|0]"
	@echo "      project .mcp.json pwt entry bound to that project (--project-root; never scans)"
	@echo "  make setup-claude-global [ALLOW_ANALYSIS=1|0]"
	@echo "      user-wide Claude Code pwt MCP entry (every directory; bound per session to the"
	@echo "      project Claude Code starts in; project .mcp.json entries override it; never scans)"
	@echo "  make claude-status [ROOT=<dir>] [TEST=1]"
	@echo "      artifacts, registration, project MCP, permissions; TEST=1 = real MCP handshake"
	@echo "  make uninstall-claude [ROOT=<dir>]  remove PWT-managed Claude files + global entry (data kept)"
	@echo "  make install-opencode    install global PWT command plugin + PWT agent (asks before overwriting)"
	@echo "  make uninstall-opencode  remove the global PWT plugin and agent"
	@echo "  make mcp [ALLOW_SCANS=1] [ALLOW_ANALYSIS=1]   MCP stdio server (stdout is protocol-only)"
	@echo "  make mcp-status      recorded MCP session presence and permission modes"
	@echo "Permission modes: read-only (default) vs scan-enabled (--allow-scans)"
	@echo "  vs analysis writes (--allow-analysis-writes, independent of scans)."
	@echo "  make install [ALLOW_ANALYSIS=0|1] [DATA_DIR=..] [PREFIX=..]"
	@echo "      one-time user install (run from the PWT checkout): global pwt launcher,"
	@echo "      OpenCode plugin/agent/ask, Claude Code /pwt, /pwt-ask + pwt subagent (if Claude"
	@echo "      Code is present; no MCP entry), desktop entry, then enable the current project"
	@echo "      (read-only unless flags given; scans run only from the dashboard)"
	@echo "  make enable ROOT=<dir> [ALLOW_ANALYSIS=0|1]"
	@echo "      register + configure one project (OpenCode entry always read-only for scans)"
	@echo "  make uninstall       remove only PWT-owned install artifacts incl. Claude/OpenCode (data kept)"
	@echo "  make purge-data      DELETE the shared data dir after confirmation (separate!)"
	@echo "  make install-shell-rc  append PREFIX to ~/.bashrc PATH if missing (opt-in)"

build: $(JAR)

$(JAR): $(shell find pwt/src pwt/pom.xml -type f 2>/dev/null) pwt/mvnw
	cd pwt && ./mvnw -B -q package -DskipTests

test:
	cd pwt && ./mvnw -B test

# Installer/integration regression tests (Python stdlib). With a compiled tree
# they also drive the real repo-add CLI and MCP server from target/classes.
test-integrations:
	@command -v python3 >/dev/null 2>&1 || { echo "test-integrations: python3 is required" >&2; exit 2; }
	cd pwt && ./mvnw -B -q compile dependency:build-classpath -Dmdep.outputFile=target/test-classpath.txt
	PWT_TEST_CLASSPATH="$(CURDIR)/pwt/target/classes:$$(cat pwt/target/test-classpath.txt)" \
		python3 -m unittest discover -s integrations/tests -v
	@if command -v node >/dev/null 2>&1; then \
		node --test integrations/tests/opencode/launch-dashboard.test.mjs; \
	else \
		echo "test-integrations: node not found; skipped OpenCode dashboard-launch tests"; \
	fi

setup:
	@mkdir -p "$(DATA_DIR)"
	@chmod 700 "$(DATA_DIR)" 2>/dev/null || true
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
	@if [ -n "$(ALLOW_SCANS)" ]; then echo "setup-opencode: ALLOW_SCANS is ignored: OpenCode never scans (dashboard-only); flag not written" >&2; fi
	python3 "$(CURDIR)/integrations/setup_opencode.py" --project-dir "$(ROOT)" \
		--data-dir "$(DATA_DIR)" \
		$(if $(ALLOW_ANALYSIS),--allow-analysis $(ALLOW_ANALYSIS))
	@echo "setup-opencode: mode handling: $(MODE_DESC)"

setup-opencode-global:
	@command -v python3 >/dev/null 2>&1 || { echo "setup-opencode-global: python3 is required" >&2; exit 2; }
	@if [ -n "$(ALLOW_SCANS)" ]; then echo "setup-opencode-global: ALLOW_SCANS is ignored: OpenCode never scans (dashboard-only); flag not written" >&2; fi
	python3 "$(CURDIR)/integrations/setup_opencode.py" --global \
		--data-dir "$(DATA_DIR)" \
		$(if $(ALLOW_ANALYSIS),--allow-analysis $(ALLOW_ANALYSIS))
	@echo "setup-opencode-global: host MCP available in every directory (unscoped entry;"
	@echo "setup-opencode-global: per-project entries override it; per-call scoping still enforced)"

setup-claude:
	@command -v python3 >/dev/null 2>&1 || { echo "setup-claude: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/setup_claude.py" --project-dir "$(ROOT)" \
		--data-dir "$(DATA_DIR)" $(if $(ALLOW_SCANS),--allow-scans $(ALLOW_SCANS)) \
		$(if $(ALLOW_ANALYSIS),--allow-analysis $(ALLOW_ANALYSIS))

# Claude Code (user-wide artifacts; project MCP via setup-claude). Never
# --allow-scans, never registers, never touches shell startup files.
install-claude:
	@command -v python3 >/dev/null 2>&1 || { echo "install-claude: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/install_claude.py" install --data-dir "$(DATA_DIR)"

setup-claude-global:
	@command -v python3 >/dev/null 2>&1 || { echo "setup-claude-global: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/install_claude.py" global --data-dir "$(DATA_DIR)" \
		$(if $(ALLOW_SCANS),--allow-scans $(ALLOW_SCANS)) \
		$(if $(ALLOW_ANALYSIS),--allow-analysis $(ALLOW_ANALYSIS))

uninstall-claude:
	@command -v python3 >/dev/null 2>&1 || { echo "uninstall-claude: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/install_claude.py" uninstall \
		$(if $(filter command line environment,$(origin ROOT)),--project-dir "$(ROOT)")

claude-status:
	@command -v python3 >/dev/null 2>&1 || { echo "claude-status: python3 is required" >&2; exit 2; }
	@python3 "$(CURDIR)/integrations/install_claude.py" status --data-dir "$(DATA_DIR)" \
		--project-dir "$(ROOT)" $(if $(filter 1,$(TEST)),--test)

mcp:
	@"$(LAUNCHER)" --data-dir "$(DATA_DIR)" $(ALLOW_FLAG) $(ANALYSIS_FLAG)

install-opencode:
	@command -v python3 >/dev/null 2>&1 || { echo "install-opencode: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/install-opencode.py"

uninstall-opencode:
	@command -v python3 >/dev/null 2>&1 || { echo "uninstall-opencode: python3 is required" >&2; exit 2; }
	python3 "$(CURDIR)/integrations/install-opencode.py" --uninstall

mcp-status:
	@if [ ! -f "$(JAR)" ]; then $(MAKE) build; fi
	@java -jar "$(JAR)" mcp-status --data-dir "$(DATA_DIR)"

install: build install-launcher install-opencode install-desktop install-path-check
	@mkdir -p "$(DATA_DIR)"
	@chmod 700 "$(DATA_DIR)" 2>/dev/null || true
	@# Claude Code user-wide artifacts (commands + read-only subagent) only when Claude
	@# Code is present; never an MCP entry, never fatal for the rest of the install.
	@if command -v claude >/dev/null 2>&1 || [ -d "$${CLAUDE_CONFIG_DIR:-$$HOME/.claude}" ]; then \
		python3 "$(CURDIR)/integrations/install_claude.py" install --data-dir "$(DATA_DIR)" \
		|| echo "install: WARNING: Claude Code artifacts not installed (see above); other components unaffected" >&2; \
	else \
		echo "install: Claude Code not found; skipped /pwt, /pwt-ask and pwt subagent (run make install-claude later)"; \
	fi
	@$(MAKE) enable ROOT="$(CURDIR)"
	@echo "install: complete."
	@echo "install: user launcher: $(PREFIX)/pwt  (try: pwt doctor)"
	@echo "install: data kept in:  $(DATA_DIR) (repos, runs, journals, exports preserved)"
	@echo "install: Claude Code project MCP is explicit: make setup-claude ROOT=/abs/project"

install-launcher:
	@mkdir -p "$(PREFIX)"
	@{ \
		echo '#!/bin/sh'; \
		echo '# PWT-OWNED launcher (managed by `make install`; safe to delete).'; \
		echo '# Linked development installation: resolves into the application root;'; \
		echo '# requires the source tree (jar updates need no config edits).'; \
		printf 'export PWT_DATA_DIR="%s"\n' "$(DATA_DIR)"; \
		printf 'exec "%s/integrations/pwt.sh" "$$@"\n' "$(CURDIR)"; \
	} > "$(PREFIX)/pwt"
	@chmod 755 "$(PREFIX)/pwt"
	@echo "install: launcher installed: $(PREFIX)/pwt"

install-desktop:
	@mkdir -p "$(APPSDIR)"
	@{ \
		echo '[Desktop Entry]'; \
		echo 'Type=Application'; \
		echo 'Name=PWT YAML Workbench'; \
		echo 'Comment=PWT assessment workbench (shared store: $(DATA_DIR))'; \
		printf 'Exec="%s/pwt" gui\n' "$(PREFIX)"; \
		echo 'Terminal=false'; \
		echo 'Categories=Development;'; \
		echo '# PWT-OWNED desktop entry (managed by `make install`; safe to delete)'; \
	} > "$(DESKTOP_FILE)"
	@if command -v update-desktop-database >/dev/null 2>&1; then \
		update-desktop-database "$(APPSDIR)" >/dev/null 2>&1 || true; \
	fi
	@echo "install: desktop entry installed: $(DESKTOP_FILE)"

install-path-check:
	@case ":$(PATH):" in \
		*":$(PREFIX):"*) echo "install: $(PREFIX) is on PATH";; \
		*) echo "install: WARNING: $(PREFIX) is not on PATH." >&2; \
			echo "install: add this line to your shell startup file, then restart the shell:" >&2; \
			echo "install:   export PATH=\"$(PREFIX):\$$PATH\"" >&2; \
			echo "install: or run this opt-in step: make install-shell-rc" >&2;; \
	esac

install-shell-rc:
	@if grep -qF '$(PREFIX)' "$(HOME)/.bashrc" 2>/dev/null; then \
		echo "install-shell-rc: already present in ~/.bashrc"; \
	else \
		printf '\nexport PATH="%s:$$PATH"\n' "$(PREFIX)" >> "$(HOME)/.bashrc"; \
		echo "install-shell-rc: appended to ~/.bashrc (restart the shell)"; \
	fi

enable:
ifeq ($(origin ROOT),default)
	$(error Usage: make enable ROOT="/absolute/project" [ALLOW_ANALYSIS=0|1])
endif
	@$(MAKE) repo-add ROOT="$(ROOT)"
	@if [ -n "$(ALLOW_SCANS)" ]; then echo "enable: ALLOW_SCANS is ignored: OpenCode never scans (dashboard-only)" >&2; fi
	@python3 "$(CURDIR)/integrations/setup_opencode.py" --project-dir "$(ROOT)" \
		--data-dir "$(DATA_DIR)" \
		$(if $(ALLOW_ANALYSIS),--allow-analysis $(ALLOW_ANALYSIS))
	@echo "enable: $(ROOT) — scans: dashboard-only analysis=$(if $(ALLOW_ANALYSIS),$(ALLOW_ANALYSIS),default) (explicit flags only)"

uninstall:
	@if [ -f "$(PREFIX)/pwt" ]; then \
		if grep -q "PWT-OWNED launcher" "$(PREFIX)/pwt" 2>/dev/null; then \
			rm -f "$(PREFIX)/pwt"; echo "uninstall: removed $(PREFIX)/pwt"; \
		else \
			echo "uninstall: kept unmanaged file: $(PREFIX)/pwt"; \
		fi; \
	else \
		echo "uninstall: not installed: $(PREFIX)/pwt"; \
	fi
	@if [ -f "$(DESKTOP_FILE)" ]; then \
		if grep -q "PWT-OWNED" "$(DESKTOP_FILE)" 2>/dev/null; then \
			rm -f "$(DESKTOP_FILE)"; echo "uninstall: removed $(DESKTOP_FILE)"; \
			if command -v update-desktop-database >/dev/null 2>&1; then \
				update-desktop-database "$(APPSDIR)" >/dev/null 2>&1 || true; \
			fi; \
		else \
			echo "uninstall: kept unmanaged file: $(DESKTOP_FILE)"; \
		fi; \
	else \
		echo "uninstall: not installed: $(DESKTOP_FILE)"; \
	fi
	@python3 "$(CURDIR)/integrations/install-opencode.py" --uninstall || true
	@python3 "$(CURDIR)/integrations/install_claude.py" uninstall || true
	@echo "uninstall: preserved by default: $(DATA_DIR) (repos, runs, journals, exports),"
	@echo "uninstall: project MCP entries (.mcp.json / opencode.json), context pointers, and the source tree."
	@echo "uninstall: remove one project's Claude entry explicitly: make uninstall-claude ROOT=/abs/project"
	@echo "uninstall: to delete application data as well, run: make purge-data"

purge-data:
	@echo "purge-data: this permanently DELETES all PWT data in: $(DATA_DIR)"
	@printf "purge-data: type YES to continue: "; read ans; \
	if [ "$$ans" = "YES" ]; then \
		rm -rf "$(DATA_DIR)"; echo "purge-data: removed $(DATA_DIR)"; \
	else \
		echo "purge-data: aborted (data kept)"; exit 1; \
	fi

clean:
	cd pwt && ./mvnw -B -q clean
