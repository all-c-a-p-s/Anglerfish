EXE := Anglerfish

ifeq ($(OS),Windows_NT)
	NAME := $(EXE).exe
	RUN := $(NAME)
else
	NAME := $(EXE)
	RUN := ./$(NAME)
endif

.PHONY: all build run clean

all: build

build:
	cargo rustc --release -- -C target-cpu=native --emit link=$(NAME)

run: build
	$(RUN)

clean:
	cargo clean
	rm -f $(NAME)
