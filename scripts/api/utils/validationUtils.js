const fs = require('fs');

function isValidAddress(address, errorMsg){
  let regex = /^\w{48}$/;
  let matchedAddress = address.match(regex)[0];
  if(!matchedAddress){
    console.log(`${errorMsg}: ${address}`);
    return false;
  }
  return true;
}

function isValidIP(host, errorMsg){
  let regex = /^\d+\.\d+\.\d+\.\d+$/;
  let matchedHost = host.match(regex)[0];
  let hasInvalidNumber = host.split('.').some(number => number < 0 || number > 255);
  if(!matchedHost || hasInvalidNumber){
    console.log(`${errorMsg}: ${host}`);
    return false;
  }
  return true;
}

function isValidPath(path, errorMsg){
  if(!fs.existsSync(path)){
    console.log(`${errorMsg}: ${path}`);
    return false;
  }
  return true;
}

function isValidPort(wsPort, errorMsg){
  if(wsPort < 1025 || wsPort > 65535){
    console.log(`${errorMsg}: ${wsPort}`);
    return false;
  }
  return true;
}

function isValidSeed(seed, errorMsg){
  let regex = /^0x[a-fA-F0-9]{64}$/;
  let matchedSeed = seed.match(regex)[0];
  if(!matchedSeed){
    console.log(`${errorMsg}: ${seed}`);
    return false;
  }
  return true;
}

function isValidValue(value, errorMsg){
  if(isNaN(value)){
    console.log(`${errorMsg}: ${value}`);
    return false;
  }
  return true;
}

module.exports = {
  isValidAddress,
  isValidIP,
  isValidPath,
  isValidPort,
  isValidSeed,
  isValidValue
};
