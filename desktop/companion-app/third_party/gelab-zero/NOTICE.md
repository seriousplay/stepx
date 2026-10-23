GELab-Zero: https://github.com/stepfun-ai/gelab-zero
Pinned revision: 7b619f6f67d2b1101021fc453cb27bd48a29e4f2

The Android adapter reuses default_skill from copilot_tools/parser_0920_summary_adv.py (MIT), ports its tab-separated action format, and provides a native Android observation/action loop with bounded history, independent verification, pause/stop and a private Step Plan client. It does not execute the upstream Python/ADB runtime on Android.
