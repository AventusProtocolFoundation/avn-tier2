FROM ubuntu:20.04

# show backtraces
ENV RUST_BACKTRACE 1

# add user
RUN useradd -m -u 1000 -U -s /bin/sh -d /subkey subkey

# add subkey binary to docker image
COPY ./subkey /usr/local/bin

USER subkey

# check if executable works in this container
RUN /usr/local/bin/subkey --version

ENTRYPOINT ["/usr/local/bin/subkey"]

