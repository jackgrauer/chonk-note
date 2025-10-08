#!/bin/bash
# Simple test to see what key codes we get for Ctrl+R
stty raw -echo
printf "Press Ctrl+R: "
od -An -tx1 -N10
stty sane
