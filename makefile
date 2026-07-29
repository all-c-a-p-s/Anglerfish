EXE := Anglerfish
TEST_EXE := AnglerfishTest

ifeq ($(OS),Windows_NT)
	NAME := $(EXE).exe
	TEST_NAME := $(TEST_EXE).exe
	RUN := $(NAME)
else
	NAME := $(EXE)
	TEST_NAME := $(TEST_EXE)
	RUN := ./$(NAME)
endif

.PHONY: all build run clean

all: build

build:
	cargo rustc --release -- -C target-cpu=native --emit link=$(NAME)

for_test:
	cargo rustc --release -- -C target-cpu=native --emit link=$(TEST_NAME)

run: build
	$(RUN)

clean:
	cargo clean
	rm -f $(NAME)
