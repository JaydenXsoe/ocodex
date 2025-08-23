#!/usr/bin/env bash 
# Currently ssh-agent doesn't work with fish terminal
# `exec (ssh-agent -c)` and `ssh-add ~/.ssh/id_ed25519_jayden` work,
# not with mac for somereason
# but does not effect the current terminal when running `sudo -E git cmd`
#ID="~/.ssh/newmac_id_ed25519"
#export GIT_SSH_COMMAND="ssh -i $ID"
#export GIT_SSH_COMMAND="ssh -i ~/.ssh/newmac_id_ed25519"
git add .
echo -e "\nEnter commit Message"
read COMMIT
git commit -m "$COMMIT"
git status
git branch -M main
git push -u origin main
