# Example data

`zh/` holds excerpts of Chinese classics in the public domain, taken from
[Project Gutenberg](https://www.gutenberg.org/) (traditional characters, with
the Project Gutenberg header and footer removed):

| File | Work | Excerpt | Gutenberg ebook |
|---|---|---|---|
| `xiyouji.txt` | 西遊記 *Journey to the West* | chapters 1–5 | [#23962](https://www.gutenberg.org/ebooks/23962) |
| `hongloumeng.txt` | 紅樓夢 *Dream of the Red Chamber* | chapters 1–5 | [#24264](https://www.gutenberg.org/ebooks/24264) |
| `sanguo.txt` | 三國志演義 *Romance of the Three Kingdoms* | chapters 1–5 | [#23950](https://www.gutenberg.org/ebooks/23950) |
| `shuihu.txt` | 水滸傳 *Water Margin* | prologue and chapters 1–5 | [#23863](https://www.gutenberg.org/ebooks/23863) |
| `shijing.txt` | 詩經 *Book of Songs* | complete | [#23873](https://www.gutenberg.org/ebooks/23873) |
| `liaozhai.txt` | 聊齋志異 *Strange Tales from a Chinese Studio* | volume 1 | [#51828](https://www.gutenberg.org/ebooks/51828) |

The English texts come from `tests/golden/` (the inputs of the upstream Python
examples). `fonts/` is created by `../fetch_fonts.sh` and is not committed.
