inotifyworld
============

Testing what we need to create an application that monitors files
created in a directory tree and feeds those to systemd-cat.

We need:

- finding existing files/dirs
- keeping track of new files and removals
- opening all files and feeding their output to a unix socket

Why?

This could be a useful toy to log local k8s node data to journald.
Containerd logs to filesystem only, and that it ephemeral. When pods go,
the logs go too.
