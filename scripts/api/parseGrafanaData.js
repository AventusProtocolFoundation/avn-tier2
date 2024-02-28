const assert = require('assert');
const fs = require('fs');
const path = require('path');

function main() {
    let args = process.argv.slice(2);
    let filename = args[0];

    if (filename == '--test') {
        return runTests();
    }
    let writeTo = path.join(path.parse(filename).dir, 'out_' + path.parse(filename).base);
    processFile(filename, writeTo);
}

function processFile(inFile, outFile) {
    let rawData = loadFromFile(inFile);
    let [headers, dataSeries] = extractDataSeries(rawData);
    createOutputFile(outFile, headers, dataSeries);
}

function loadFromFile(filename) {
    var contents = fs.readFileSync(filename, 'utf8');
    return tokenize(contents, '\n');
}

/*
Raw Data Format:
- we receive a series of lines with the same format
- the first line contains the headers. All the others are data lines.
- in each line, we have first a time field, indicating the time of the data sample
- this is followed by a number of columns equal to the number of nodes we were running in the trial.
- fields are separated by ';'
- each line corresponds to a different series, which means only one entry should have a non-null value
- however, because we don't have confirmation of the data format, the script extracts all non-null values in a line
*/
function extractDataSeries(fileContents) {
    let [headers, data] = extractHeaders(fileContents);
    let result = initializeResultArray(fileContents, headers);

    for (const line of data) {
        let values = getDataColumns(line);
        addNonNullValuesToSeries(result, values);
    }

    return [headers, result];
}

function createOutputFile(filename, header, data) {
    let file = fs.openSync(filename, 'w');

    fs.writeSync(file, header + '\n');
    let currentSampleIndex = 0;

    // TODO [TYPE: refactoring][PRI: low]: make this more functional in the future, if needed
    let seriesLength = maximumSeriesLength(data);

    for (let i = 0; i < seriesLength; i++) {
        for (const series of data) {
            if (currentSampleIndex < series.length) {
                fs.writeSync(file, series[currentSampleIndex] + ',');
            } else {
                fs.writeSync(file, ',');
            }
        }
        fs.writeSync(file, '\n');
        currentSampleIndex++;
    }

    fs.closeSync(file);
}

function tokenize(text, separator) {
    let trimmedText = removeLastSeparator(text, separator);
    return trimmedText.split(separator);
}

function removeLastSeparator(s, sep) {
    if (s[s.length-1] == sep) {
        return s.slice(0, s.length-1);
    } else {
        return s;
    }
}

function extractHeaders(fileContents) {
    let headers = fileContents[0];

    let titles = [];
    for (const header of tokenize(headers, ';')) {
        titles.push(header.trim().replace(/"/g, '',));
    }

    return [titles.slice(1), fileContents.slice(1)];
}

function initializeResultArray(fileContents, headers) {
    let numberOfSeries = headers.length;
    let result = [];
    for (let i = 0; i < numberOfSeries; i++) {
        result.push([]);
    }
    return result;
}

function getDataColumns(lineData) {
    return lineData.split(';').slice(1);
}

function addNonNullValuesToSeries(result, values) {
    values.map(t => t.trim()).forEach((value, index) => {
        if (value != 'null') {
            result[index].push(value);
        }
    });
}

function maximumSeriesLength(dataSeries) {
    return dataSeries.map(series => series.length).reduce((maxLength, seriesLength) => Math.max(maxLength, seriesLength));
}

// --------------------- Tests -----------------------

function getTestData() {
    const contents = '"Time";"bandwidth_download";"bandwidth_download";"bandwidth_download";\n' +
        '"15:19:35";null;null;null\n' +
        '"15:19:35";31749;null;null\n' +
        '"15:19:35";null;85995;null\n' +
        '"15:19:35";null;null;85995\n' +
        '"15:19:35";null;90000;null';
    return tokenize(contents, '\n');
}

function createTestFile() {
    let inFile = 'test.csv';
    const rawData = getTestData();
    let file = fs.openSync(inFile, 'w');
    for (const line of rawData) {
        fs.writeSync(file, line + '\n');
    }
    fs.closeSync(file);
}

function runTests() {
    runTestExtractHeaders();
    runTestExtractDataSeries();
    runTestCreateOutputFile();
    runMainTest();
}

function runTestExtractHeaders() {
    let s = ['"Time";"bandwidth_download";"bandwidth_download";"bandwidth_download";', 'a,b,c,d,e'];
    let [headers, data] = extractHeaders(s);

    auxTestHeaders(headers);
    assert.equal(data, 'a,b,c,d,e');
}

function runTestExtractDataSeries() {
    let rawData = getTestData();
    let [, dataSeries] = extractDataSeries(rawData);

    assert.equal(dataSeries.length, 3, 'Number of Series: ' + dataSeries);
    assert.equal(dataSeries[0].length, 1, 'Length of series 1');
    assert.equal(dataSeries[1].length, 2, 'Length of series 2');
    assert.equal(dataSeries[2].length, 1, 'Length of series 3');

    assert.equal(dataSeries[0][0], 31749, 'Data of series 1');
    assert.equal(dataSeries[1][0], 85995, 'Data of series 2');
    assert.equal(dataSeries[1][1], 90000, 'Data of series 2');
    assert.equal(dataSeries[2][0], 85995, 'Data of series 3');
}

function runTestCreateOutputFile() {
    let outFile = 'out_test.csv';
    let rawData = getTestData();
    let [headers, series] = extractDataSeries(rawData);

    createOutputFile(outFile, headers, series);

    let test_results = loadFromFile('out_test.csv');
    let dataSeries = test_results.slice(1).map(s => tokenize(s, ','));

    auxTestOutput(dataSeries);
}

function runMainTest() {
    createTestFile();
    let inFile = 'test.csv';
    let outFile = 'out_test.csv';

    processFile(inFile, outFile);
    let test_results = loadFromFile('out_test.csv');
    let headers = tokenize(test_results[0], ',');
    let dataSeries = test_results.slice(1).map(s => tokenize(s, ','));

    auxTestHeaders(headers);
    auxTestOutput(dataSeries);
}

function auxTestHeaders(headers) {
    assert.equal(headers.length, 3);
    for (let i = 0; i < 3; i++) {
        assert.equal(headers[i], 'bandwidth_download', 'Header value ' + i);
    }
}

function auxTestOutput(dataSeries) {
    assert.equal(dataSeries.length, 2, 'Number of Samples in Series');
    assert.equal(dataSeries[0].length, 3, 'Length of series 1');
    assert.equal(dataSeries[1].length, 3, 'Length of series 2');

    assert.equal(dataSeries[0][0], 31749, 'Data of series 1');
    assert.equal(dataSeries[0][1], 85995, 'Data of series 1');
    assert.equal(dataSeries[0][2], 85995, 'Data of series 1');
    assert.equal(dataSeries[1][0], '', 'Data of series 2');
    assert.equal(dataSeries[1][1], 90000, 'Data of series 2');
    assert.equal(dataSeries[1][2], '', 'Data of series 2');
}

main();
