.PHONY: all plan score render check clean

all: plan score render

plan:
	python3 plan.py

score:
	python3 score.py

render:
	node render.mjs

# prove seek(t) is pure: revisit frames and compare bytes
check:
	node render.mjs --check

# one frame: make still T=42.5
still:
	node render.mjs --still $(T)

clean:
	rm -rf frames out/*.png
