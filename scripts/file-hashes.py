# 교차검증 전후에 파일이 바뀌지 않았는지 비교한다.
# 사용: python scripts/file-hashes.py > before.txt  (검토 후) python scripts/file-hashes.py | diff before.txt -
import hashlib
import os

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
skip = {".git", ".claude", "node_modules", "target", "dist", "backups"}
for folder, dirs, files in os.walk(root):
    dirs[:] = sorted(d for d in dirs if d not in skip)
    for name in sorted(files):
        path = os.path.join(folder, name)
        with open(path, "rb") as f:
            print(hashlib.sha256(f.read()).hexdigest(), os.path.relpath(path, root))
