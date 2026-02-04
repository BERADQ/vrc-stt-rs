#!/usr/bin/env node

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

function checkPackageVersions() {
    try {
         // Run cargo metadata to get package information
         const output = execSync('cargo metadata --format-version 1', {
             cwd: process.cwd(),
             encoding: 'utf-8',
             maxBuffer: 10 * 1024 * 1024 // Increase buffer to 10MB
         });

        const metadata = JSON.parse(output);
        const packages = metadata.packages;

         // Find the three specific packages
         const targetPackages = ['backend', 'frontend', 'common'];
         const foundVersions = {};

         packages.forEach(pkg => {
             if (targetPackages.includes(pkg.name)) {
                 foundVersions[pkg.name] = pkg.version;
             }
         });

         // Check if all three packages were found
         for (const pkgName of targetPackages) {
             if (!foundVersions[pkgName]) {
                 console.error(`Error: Package '${pkgName}' not found in workspace`);
                 process.exit(1);
             }
         }

        // Get unique versions
        const uniqueVersions = [...new Set(Object.values(foundVersions))];

        // Check if all versions are the same
        if (uniqueVersions.length !== 1) {
            console.error('Error: Package versions do not match!');
            Object.entries(foundVersions).forEach(([name, version]) => {
                console.error(`${name}: ${version}`);
            });
            process.exit(1);
        }

        console.log('All package versions match:', uniqueVersions[0]);
        Object.entries(foundVersions).forEach(([name, version]) => {
            console.log(`${name}: ${version}`);
        });

        return uniqueVersions[0]; // Return the version for packaging
    } catch (error) {
        console.error('Error running cargo metadata:', error.message);
        process.exit(1);
    }
}

function getPlatformInfo() {
    // Map Node.js platform to system name
    const platformMap = {
        'win32': 'windows',
        'linux': 'linux',
        'darwin': 'macos',
        'freebsd': 'freebsd'
    };

    // Map Node.js arch to CPU architecture name
    const archMap = {
        'x64': 'x64',
        'arm64': 'arm64',
        'ia32': 'x86',
        'arm': 'arm'
    };

    const osName = platformMap[process.platform] || process.platform;
    const archName = archMap[process.arch] || process.arch;

    return { osName, archName };
}

function buildBackend(backendType) {
    console.log(`Building backend (${backendType})...`);

    let buildCmd;
    if (backendType === 'vulkan') {
        // Vulkan uses default features
        buildCmd = 'cargo build --release -p backend';
    } else if (backendType === 'cuda') {
        // CUDA needs explicit feature and no-default-features
        buildCmd = 'cargo build --release -p backend --features cuda --no-default-features';
    } else {
        throw new Error(`Unknown backend type: ${backendType}`);
    }

    try {
        execSync(buildCmd, {
            stdio: 'inherit',
            cwd: process.cwd()
        });
        console.log(`Backend (${backendType}) build completed successfully.`);
    } catch (error) {
        console.error(`Error building backend (${backendType}):`, error.message);
        process.exit(1);
    }
}

function buildFrontend() {
    console.log('Building frontend binary...');
    try {
        execSync('cargo build --release -p frontend', {
            stdio: 'inherit',
            cwd: process.cwd()
        });
        console.log('Frontend build completed successfully.');
    } catch (error) {
        console.error('Error building frontend:', error.message);
        process.exit(1);
    }
}

function createPackage(backendType, version, osName, archName) {
    console.log(`\n========== Packaging ${backendType.toUpperCase()} version ==========`);

    // Determine binary extension based on platform
    const exeExt = process.platform === 'win32' ? '.exe' : '';

    // Expected binary paths after build
    const backendBinaryPath = path.join('target', 'release', `backend${exeExt}`);
    const frontendBinaryPath = path.join('target', 'release', `frontend${exeExt}`);

    // Check if binaries exist
    if (!fs.existsSync(backendBinaryPath)) {
        console.error(`Backend binary not found at: ${backendBinaryPath}`);
        process.exit(1);
    }
    if (!fs.existsSync(frontendBinaryPath)) {
        console.error(`Frontend binary not found at: ${frontendBinaryPath}`);
        process.exit(1);
    }

    // Create archive name: {backendType}-{os}-{arch}-{version}.7z
    const archiveName = `${backendType}-${osName}-${archName}-${version}.7z`;
    const archivePath = path.join('target', archiveName);

    // Create temporary directory with same name as archive (without extension)
    const tempDir = path.join('target', `${backendType}-${osName}-${archName}-${version}`);
    if (!fs.existsSync(tempDir)) {
        fs.mkdirSync(tempDir);
    }

    // Ensure temp directory is clean
    if (fs.existsSync(tempDir)) {
        execSync(`rm -rf "${tempDir}"`);
    }
    fs.mkdirSync(tempDir);

    console.log(`Created temporary directory: ${tempDir}`);

    // Copy binaries to temp directory
    // Backend: copy without renaming
    const backendDest = path.join(tempDir, `backend${exeExt}`);
    execSync(`cp "${backendBinaryPath}" "${backendDest}"`);
    console.log(`Copied backend to: ${backendDest}`);

    // Frontend: copy and rename to vrc-stt-rs
    const frontendDest = path.join(tempDir, `vrc-stt-rs${exeExt}`);
    execSync(`cp "${frontendBinaryPath}" "${frontendDest}"`);
    console.log(`Copied frontend to: ${frontendDest} (renamed from frontend)`);

    // Check if 7z is available
    try {
        execSync('7z', { stdio: 'pipe' });
    } catch (error) {
        console.error('7z command not found. Please install 7-zip.');
        // Cleanup temp directory before exiting
        execSync(`rm -rf "${tempDir}"`);
        process.exit(1);
    }

    // Create 7z archive
    console.log(`Creating archive: ${archivePath}`);
    try {
        execSync(`7z a "${archivePath}" "${tempDir}"/*`, {
            stdio: 'inherit'
        });
        console.log(`Archive created successfully: ${archivePath}`);
    } catch (error) {
        console.error('Error creating archive:', error.message);
        // Cleanup temp directory
        execSync(`rm -rf "${tempDir}"`);
        process.exit(1);
    }

    // Cleanup temp directory
    execSync(`rm -rf "${tempDir}"`);
    console.log('Temporary directory cleaned up.');
}

function buildAndPackage(targetBackend) {
    console.log('Starting build and packaging process...');

    // Check versions first
    const version = checkPackageVersions();

    // Get platform info
    const { osName, archName } = getPlatformInfo();
    console.log(`Platform: ${osName}, Architecture: ${archName}`);

    // Build frontend (only needs to be built once, shared by both backends)
    buildFrontend();

    const completedPackages = [];

    if (targetBackend === null || targetBackend === 'vulkan') {
        // Build and package Vulkan version
        console.log('\n========== Building VULKAN backend ==========');
        buildBackend('vulkan');
        createPackage('vulkan', version, osName, archName);
        completedPackages.push(`target/vulkan-${osName}-${archName}-${version}.7z`);
    }

    if (targetBackend === null || targetBackend === 'cuda') {
        // Build and package CUDA version
        console.log('\n========== Building CUDA backend ==========');
        buildBackend('cuda');
        createPackage('cuda', version, osName, archName);
        completedPackages.push(`target/cuda-${osName}-${archName}-${version}.7z`);
    }

    console.log('\n========================================');
    console.log('All packaging completed successfully!');
    console.log('Output files (in target/ directory):');
    completedPackages.forEach(pkg => console.log(`  - ${pkg}`));
    console.log('========================================');
}

// Parse command line arguments
const args = process.argv.slice(2);
let targetBackend = null;

if (args.length > 0) {
    const arg = args[0].toLowerCase();
    if (arg === 'cuda') {
        targetBackend = 'cuda';
        console.log('Target: CUDA only');
    } else if (arg === 'vulkan') {
        targetBackend = 'vulkan';
        console.log('Target: Vulkan only');
    } else {
        console.error(`Unknown backend type: ${arg}`);
        console.error('Usage: node release.js [cuda|vulkan]');
        console.error('  - no argument: build both cuda and vulkan');
        console.error('  - cuda: build cuda only');
        console.error('  - vulkan: build vulkan only');
        process.exit(1);
    }
} else {
    console.log('Target: All backends (cuda + vulkan)');
}

// Run the build and packaging
buildAndPackage(targetBackend);
