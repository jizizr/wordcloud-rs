"""Write golden reference data from the legacy Python implementation."""
import json, random, sys
from wordcloud import WordCloud
out = sys.argv[1]
seeds = [0, 1, 42, 123456789, 2**40 + 5, -3]
rng = []
for s in seeds:
    r = random.Random(s)
    rng.append({"seed": s,
                "random": [r.random() for _ in range(20)],
                "randint": [r.randint(0, n) for n in (1, 2, 49, 255, 1000, 2**31, 7)],
                "uniform": [r.uniform(0.2, 1) for _ in range(5)],
                "bits": [r.getrandbits(k) for k in (1, 7, 32, 33, 64)]})
json.dump(rng, open(f"{out}/rng.json", "w"))
corpora = ["examples/alice.txt", "examples/constitution.txt", "examples/a_new_hope.txt", "test/unicode_text.txt"]
opts = [{}, {"collocations": False}, {"normalize_plurals": False}, {"include_numbers": True, "min_word_length": 3},
        {"collocation_threshold": 3}, {"regexp": r"\w+"}]
cases = []
for c in corpora:
    text = open(c, encoding="utf-8").read()
    for o in opts:
        counts = WordCloud(**o).process_text(text)
        cases.append({"corpus": c, "opts": o, "counts": list(counts.items())})
json.dump(cases, open(f"{out}/text.json", "w"), ensure_ascii=False)
print(len(rng), "rng seeds,", len(cases), "text cases")
