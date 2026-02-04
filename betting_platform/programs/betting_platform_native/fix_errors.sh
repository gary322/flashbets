#\!/bin/bash
# Find all error codes that need fixing
grep -n "= 6132," src/error.rs | grep -v "VisibleSizeTooLarge" | head -1
grep -n "= 6133," src/error.rs | head -1
grep -n "= 6134," src/error.rs | head -1
grep -n "= 6135," src/error.rs | head -1
grep -n "= 6136," src/error.rs | head -1
grep -n "= 6137," src/error.rs | head -1
grep -n "= 6138," src/error.rs | head -1
grep -n "= 6139," src/error.rs | head -1
grep -n "= 6140," src/error.rs | head -1
grep -n "= 6142," src/error.rs | head -1
grep -n "= 6143," src/error.rs | head -1
