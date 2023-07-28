CMD=cargo
BUILD=$(CMD) build --release
RUN=$(CMD) run
INSTALL=$(CMD) install
CLEAN=$(CMD) clean
TEST=$(CMD) test
DOCKER=docker
DBUILD=$(DOCKER) build
DTAG= $(DOCKER) tag
DPUSH= $(DOCKER) push

BINARY_NAME=premium-rs
BINARY_VERSION=$(shell git rev-parse HEAD)
BINARY_UNIX=$(BINARY_NAME)
TAG_LOCAL = $(BINARY_NAME):$(BINARY_VERSION)
TAG_HUB = bikertales/$(BINARY_NAME):$(BINARY_VERSION)

.PHONY: build # - Builds linux arch binary
build:
	$(BUILD)

.PHONY: install  # - Installs go service 
install:
	$(INSTALL)

.PHONY: run # - Runs the service
run:
	$(RUN)

.PHONY: dbuild  # - Builds docker image
dbuild: build
	$(DBUILD) --platform linux/amd64 . -t $(TAG_LOCAL)

.PHONY: dtag # - Tags local image to docker hub tag
dtag: dbuild
	$(DTAG) $(TAG_LOCAL) $(TAG_HUB)

.PHONY: dpush # - Pushes tag to docker hub
dpush: dtag
	$(DPUSH) $(TAG_HUB)

.PHONY: tasks
tasks:
	@grep '^.PHONY: .* #' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1 \2/' | expand -t20

