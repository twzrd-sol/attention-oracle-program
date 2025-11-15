import bs58 from 'bs58';

const base64Data = "ITOthiOMw/gBARr45+bhkE7X85/NYmoVsREGe3qI8hyMfDsfiqdeUIEWabQWMVfW/HJLxVbbMidbpbVMIfe4v2cGGN0pZ6nY2RjTljYpU5dKtev2wG1uh3+oO2GhegWizTQY4HVgUd8HkYgaNHuVE2Bu7v5qqELbDs4lJUUWnbBWGbOaSwi7dq/CAAD/";
const data = Buffer.from(base64Data, 'base64');

console.log('Data length:', data.length);
console.log('\nDiscriminator (bytes 0-7):', data.slice(0, 8).toString('hex'));
console.log('Borsh prefix (bytes 8-9):', data.slice(8, 10).toString('hex'));

// Admin starts at byte 10
const adminBytes = data.slice(10, 42);
const admin = bs58.encode(adminBytes);
console.log('\nAdmin (bytes 10-41):', admin);

// Publisher starts at byte 42
const publisherBytes = data.slice(42, 74);
const publisher = bs58.encode(publisherBytes);
console.log('Publisher (bytes 42-73):', publisher);

// Paused flag at byte 74
const paused = data[74];
console.log('Paused (byte 74):', paused === 1);

const oracleAuth = '87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy';
console.log('\n=== Checking Oracle Authority ===');
console.log('Oracle Authority:', oracleAuth);
console.log('Admin matches?', admin === oracleAuth);
console.log('Publisher matches?', publisher === oracleAuth);
