#!/usr/bin/env bash

# Sets up a throwaway repo for manually trying out gatomic: a few commits,
# a modified tracked file with two separated hunks, and an untracked file —
# enough to exercise all three panes (files, hunks, commits).
function basic(){
  rm -rf "$1" || true
  mkdir -p "$1"
  cd "$1"

  git init -b main
  git config user.email "test@example.com"
  git config user.name "tester"

  mkdir src
  seq 1 30 > src/numbers.txt
  echo "hello" > src/greeting.txt
  git add src/numbers.txt src/greeting.txt
  git commit -m 'Initial'

  echo "world" >> src/greeting.txt
  git add src/greeting.txt
  git commit -m 'Greeting: add world'

  sed -i.bak '2s/.*/CHANGED2/' src/numbers.txt
  sed -i.bak '28s/.*/CHANGED28/' src/numbers.txt
  rm -f src/numbers.txt.bak

  echo "new file content" > src/todo.txt

  git status
  git log --oneline
}

set -e
"$1" "tmp/$2"
