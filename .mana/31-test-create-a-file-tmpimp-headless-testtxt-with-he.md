---
id: '31'
title: 'test: create a file /tmp/imp-headless-test.txt with ''headless works'''
slug: test-create-a-file-tmpimp-headless-testtxt-with-he
status: open
priority: 2
created_at: '2026-03-24T02:40:08.572169Z'
updated_at: '2026-03-24T02:40:08.572169Z'
labels:
- test
verify: cat /tmp/imp-headless-test.txt 2>/dev/null | grep -q 'headless works'
---

Simple test unit: write 'headless works' to /tmp/imp-headless-test.txt using the write tool.
