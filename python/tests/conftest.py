import os
import sys

# Put the python/ dir on sys.path so tests can `import loader` / `from bi import q1`.
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
