FROM ubuntu:20.04

# show backtraces
ENV RUST_BACKTRACE 1

RUN apt-get update && apt-get install -y openssl libssl-dev apt-transport-https ca-certificates curl
EXPOSE 9933 9944 9615 30333
COPY ./avn-node /avn/
COPY ./wbuild/node-runtime/node_runtime.compact.wasm /avn/wbuild/

RUN ls /avn
RUN /avn/avn-node --version
RUN mkdir /avn/node-data/

ENTRYPOINT [ "/avn/avn-node", \
  "--base-path", "/avn/node-data", \
  "--port", "30333", \
  "--rpc-port", "9933", \
  "--ws-port", "9944", \
  "--prometheus-port", "9615" ]