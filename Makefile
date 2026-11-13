# PWT (Phase 2 rebuild) — entry points.
#
#   make start            build if needed, then launch the desktop GUI
#   make build            build target/pwt-0.2.0.jar (skips tests)
#   make test             full JUnit suite (70 tests)
#   make discover ROOT=<dir>   headless YAML discovery (JSON on stdout)
#   make scan ROOT=<dir> FILES="a.yaml sub/b.yml" [ADAPTERS=...] [DATADIR=...]
#                         headless scan of selected files (result JSON on stdout)
#   make mcp              MCP stdio server (stdout is MCP-protocol-only)
#
# Requires JDK 21 and (for `start`) a display. First build needs network
# access to Maven Central; afterwards dependencies are cached in ~/.m2.

JAVA_HOME ?= /usr/lib/jvm/java-21-openjdk-amd64
export JAVA_HOME
export PATH := $(JAVA_HOME)/bin:$(PATH)

JAR := pwt/target/pwt-0.3.0.jar
SOURCES := $(shell find pwt/src pwt/pom.xml -type f 2>/dev/null)

.PHONY: start build test discover scan mcp clean help

start: $(JAR)
	java -jar $(JAR) gui

build: $(JAR)

$(JAR): $(SOURCES) pwt/mvnw
	cd pwt && ./mvnw -B -q package -DskipTests

test:
	cd pwt && ./mvnw -B test

discover: $(JAR)
ifndef ROOT
	$(error Usage: make discover ROOT=<directory>)
endif
	java -jar $(JAR) discover --root "$(ROOT)"

scan: $(JAR)
ifndef ROOT
	$(error Usage: make scan ROOT=<dir> FILES="a.yaml sub/b.yml" [ADAPTERS=yamllint,gitleaks] [DATADIR=<dir>])
endif
ifndef FILES
	$(error Usage: make scan ROOT=<dir> FILES="a.yaml sub/b.yml" [ADAPTERS=yamllint,gitleaks] [DATADIR=<dir>])
endif
	java -jar $(JAR) scan --root "$(ROOT)" $(foreach f,$(FILES),--file "$(f)") $(if $(ADAPTERS),--adapter "$(ADAPTERS)") $(if $(DATADIR),--data-dir "$(DATADIR)")

mcp: $(JAR)
	java -jar $(JAR) mcp

clean:
	cd pwt && ./mvnw -B -q clean

help:
	@echo "Targets: start build test discover scan mcp clean"
	@echo "  make start              launch the desktop GUI"
	@echo "  make discover ROOT=DIR  headless YAML discovery"
	@echo "  make scan ROOT=DIR FILES=\"a.yaml sub/b.yml\"  headless scan of selected files"
	@echo "  make mcp                MCP stdio server"
