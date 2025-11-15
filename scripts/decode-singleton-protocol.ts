import bs58 from 'bs58';

const base64Data = "ITOthiOMw/gBAZEWHDNsfngjYE8KAisvEGBA2BakeQ7J8rowRTvbG1mZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADZOtkM0mYmjjsn4ziaote/jrLl5X7aJjibetdD/wNspIgaNHuVE2Bu7v5qqELbDs4lJUUWnbBWGbOaSwi7dq/CAAD9";
const data = Buffer.from(base64Data, 'base64');

console.log('Data length:', data.length);
const adminBytes = data.slice(10, 42);
const admin = bs58.encode(adminBytes);
console.log('Admin:', admin);

const publisherBytes = data.slice(42, 74);
const publisher = bs58.encode(publisherBytes);
console.log('Publisher:', publisher);

const paused = data[74];
console.log('Paused:', paused === 1);

const oracleAuth = '87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy';
console.log('\nOracle Authority:', oracleAuth);
console.log('Admin matches?', admin === oracleAuth);
console.log('Publisher matches?', publisher === oracleAuth);
