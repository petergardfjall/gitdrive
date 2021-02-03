#!/bin/bash

set -e

function log {
    echo "[$(date +%H:%M:%S)] ${1}"
}

function ping_remote {
    # # TODO: simulate offline
    # return 1
    git ls-remote --exit-code -h > /dev/null 2>&1
}

function sync {
    watch_dir="${1}"
    pushd ${watch_dir} > /dev/null
    watcher="$(hostname):$(pwd)"

    log "syncing ..."

    if [ "$(git branch --show-current)" != "master" ]; then
        git checkout master
    fi

    local_changes=$(git status --porcelain)
    if [ -n "${local_changes}" ]; then
        log "commiting local changes ..."
        # add any already tracked files that have been modified
        git ls-files --modified | xargs git add
        # git add --ignore-removal .
        git commit -m "${watcher}: $(date +%H:%M:%S)"
    else
        log "no local changes ..."
    fi

    # check network connectivity
    if ! ping_remote; then
        log "remote unreachable, cannot rebase local changes ..."
        return 0
    fi

    log "fetching remote changes ..."
    git fetch origin master
    local_modtime=$(git show -s --format=%ct HEAD)
    remote_modtime=$(git show -s --format=%ct origin/master)
    log "local modtime: ${local_modtime}"
    log "remote modtime: ${remote_modtime}"
    if [ "$(git rev-list --count master..origin/master)" = "0" ]; then
        log "no remote changes"
    else
        log "rebasing onto remote changes ..."
        git rebase origin/master || true
        while [ -n "$(git diff --name-only --diff-filter=U)" ]; do
            log "resolving conflicts ..."
            for f in $(git diff --name-only --diff-filter=U); do
                log "resolving conflict in ${f} ..."
                git show :1:${f} > ${f}.common
                git show :2:${f} > ${f}.ours
                git show :3:${f} > ${f}.theirs

                # choose either local changes ("theirs") or remote changes
                # ("ours") based on timestamp of changes
                # ours_timestamp=$(git show -s --format=%ct origin/master)
                # theirs_timestamp=$(git show -s --format=%ct master)
                # log "ours timestamp: ${ours_timestamp}"
                # log "theirs timestamp: ${theirs_timestamp}"
                # if [ "${ours_timestamp}" -gt "${theirs_timestamp}" ]; then
                #     log "resolving conflict in favor of remote changes"
                #     strategy="--ours"
                # else
                #     log "resolving conflict in favor of local changes"
                #     strategy="--theirs"
                # fi

                strategy="--theirs"
                git merge-file -p ${strategy} ${f}.ours ${f}.common ${f}.theirs > ${f}
                git add ${f} # mark resolved
                rm ${f}.ours ${f}.common ${f}.theirs
            done

            git rebase --continue
        done
    fi

    # TODO: only if there is network connectivity
    if [ "$(git rev-list --count origin/master..master)" -gt "0" ]; then
        log "pushing local changes ..."
        git push origin master
    fi

    popd > /dev/null
}

[ -z "${1}" ] && echo "error: no watch dir" && exit 1
watch_dir="${1}"

# while true; do
    sync ${watch_dir}
    echo
# sleep 20s
# done
