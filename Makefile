.PHONY: all test check install dist deps c
all test check install dist deps:
	$(MAKE) -C rust $@
c:
	$(MAKE) -C c
