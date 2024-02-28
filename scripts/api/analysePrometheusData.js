const fs = require('fs');

const SUM = (a,b) => a + b;
const SQUARE = n => n * n;
const MIN = (a,b) => (a < b) ? a : b;
const MAX = (a,b) => (a > b) ? a : b;
const STD_DEV_RESOLUTION = 0.25;

function quantize(value, reference, resolution) {
  let multiple = value / reference;
  let invertSign = (multiple < 0) ? -1 : 1;
  multiple = Math.abs(multiple);
  return invertSign * Math.ceil(multiple / resolution) * resolution;
}

function parseDataFile(filename) {
  let rawData = JSON.parse(fs.readFileSync(filename, 'utf8'));
  let status = (rawData.status == 'success');
  console.log(`Status: ${status}`); 

  if (!status) {
    process.exit(1);
  }
  return rawData.data.result;
}

function computeSeriesStats(series) {
  let valueSeries = series.values.map(tuple => parseInt(tuple[1]));
  let sum = valueSeries.reduce(SUM);
  let count = valueSeries.length;
  let max = valueSeries.reduce(MAX);
  let min = valueSeries.reduce(MIN);
  let average = sum / count;
  let variance = valueSeries.map(sample => SQUARE(sample - average)).reduce(SUM) / count;
  let stdDev = Math.sqrt(variance);
  return {sum, count, min, max, average, stdDev};
}

// Don't assume the series already has the stats, since that is dependent on a previous step having been called
// This function is more testable if we pass average and std Dev as independent arguments
function normalizeSeries(series, average, stdDev) {
  let baseTime = series.values[0][0];
  return series.values.map(tuple => {
    return {time: tuple[0] - baseTime, value: tuple[1], box: quantize(tuple[1] - average, stdDev, STD_DEV_RESOLUTION)};
  });
}

function extractPeaks(series, minimumStdDevBox) {
  return series.filter(series => series.box >= minimumStdDevBox);
}

function showProcessResults(allSeries, normalizedSeries, filteredSeries, index) {
  console.log(`\n------ Series ${index} ------\n`);
  console.log('Stats:', allSeries[index].stats);
  
  let values = allSeries[index].values.map(sample => (sample[1]));
  console.log('\nRaw Values:', '\n' + values.join('\n'));
  console.log('\nNormalized Values:\n', normalizedSeries[index]);
  console.log('\nPeaks 1 or more StdDev away:\n', filteredSeries[index]);
}

function computeAverageSeries(allSeries) {
  let averageSeries = [];
  let nSeries = allSeries.length;
  let timeSeries = allSeries[0].map(sample => sample.time);

  for (let time = 0; time < timeSeries.length; time++) {
    let sum = allSeries.map(series => parseInt(series[time].value)).reduce(SUM);
    averageSeries.push({time: timeSeries[time], value: sum / nSeries});
  }

  return averageSeries;
}

// returns decile partitions, that is, the 10%, 20%, ... 90%, 100% percentiles 
// (the last one is not needed, but it is useful to have a measure of the maximum when plotting these values in a chart)
// The result has 9 (10) values, dividing the input set at equidistant points
// This is rough and not very polished. 
// The kth decile is computed as the first element that is above the lowest k * 10% of the data sample
// Usually, this will not be a whole number, so the result is just truncated. 
function computeDeciles(series) {
  series.sort((a,b) => parseFloat(a.value) - parseFloat(b.value));
  let decileSeries = [];
  let percentileMultiple = series.length / 10; 
  for (let i = 1; i <= 10; i++) {
    let index = Math.min(series.length - 1, parseInt(percentileMultiple * i));
    let value = series[index];
    let percentile = i * 10;
    decileSeries.push({percentile, index, value}); 
  }
  return decileSeries;
}

function main() {
  if (process.argv.length <= 2) {
    console.log('Must provide an input argument: path to data file');
    console.log('Exiting');
    process.exit(1);
  }
  let filename = process.argv[2];
  let allSeries = parseDataFile(filename);  

  let normalizedSeries = [];
  let filteredSeries = [];

  allSeries.forEach((series, index) => {
    series.stats = computeSeriesStats(series);
    let normalSeries = normalizeSeries(series, series.stats.average, series.stats.stdDev);
    normalizedSeries.push(normalSeries);     
    filteredSeries.push(extractPeaks(normalSeries, 1));

    console.log(`Series ${index}: Average: ${series.stats.average} --- Max: ${series.stats.max}`);
  });

  let averageSeries = computeAverageSeries(normalizedSeries);
  console.log('\n------ Average Series ------\n');
  console.log(averageSeries.map(sample => sample.time + ': ' + parseFloat(sample.value)).join('\n'));

  let deciles = computeDeciles(averageSeries);
  console.log('Deciles: %o', deciles);

  // Show a single example:
  showProcessResults(allSeries, normalizedSeries, filteredSeries, 0);
}

main();

