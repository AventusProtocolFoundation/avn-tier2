# System Dashboard
System Dashboard is taking Grafana service as a platform to collect statistics from Substrate nodes, and display it.

## Contents:
  * [Install Grafana](#install-grafana)
  * [Install JSON Data Source](#install-json-data-source-plugin)
  * [Add Data Sources](#add-data-sources)
  * [Import Dashboard](#build-or-import-dashboard)
  
TODO: Customise legend labels to data source names.

## Install Grafana

  Following the instructions on [here](https://grafana.com/docs/grafana/latest/installation/) to install Grafana.

  ### Install the latest stable release of Grafana:
  ```
  sudo apt-get install -y apt-transport-https
  sudo apt-get install -y software-properties-common wget
  wget -q -O - https://packages.grafana.com/gpg.key | sudo apt-key add -
  sudo add-apt-repository "deb https://packages.grafana.com/oss/deb stable main"
  sudo apt-get update
  sudo apt-get install grafana
  ```

  ### Start the server with systemd
  ```
  sudo systemctl daemon-reload
  sudo systemctl start grafana-server
  sudo systemctl status grafana-server
  ```

## Install JSON Data Source Plugin
  ```
  sudo grafana-cli plugins install simpod-json-datasource
  service grafana-server restart
  ```

## Add Data Sources

  1. Browse to http://localhost:3000
  2. First time login in as: 
      username: admin 
      password admin
  3. Go to Configuration -> Data Sources -> Add data source -> Others
     Select JSON data source
  4. Set the url of the running server, such as http://localhost:9955\
     Note: This port number should be the one configured when start the node, --grafana-port
  5. Click Save & Test

## Import Dashboard

  1. Go to Create -> Import and click the "Upload .json file" button
  2. Select the "grafana-testnet.json" file from ./dashboard/ folder, change uid if needed
  3. Click "Import" button to import the dashboard.
