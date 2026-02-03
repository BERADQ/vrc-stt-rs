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

function buildAndPackage() {
    console.log('Starting build and packaging process...');

    // Check versions first
    const version = checkPackageVersions();

    // Build both binaries in release mode using -p flag to specify packages
    console.log('Building backend binary with cargo build --release -p backend...');
    try {
        execSync('cargo build --release -p backend', {
            stdio: 'inherit',
            cwd: process.cwd()
        });
        console.log('Backend build completed successfully.');
    } catch (error) {
        console.error('Error building backend:', error.message);
        process.exit(1);
    }
    
    console.log('Building frontend binary with cargo build --release -p frontend...');
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

    // Create archive name using version
    const archiveName = `vrc-stt-rs-${version}.7z`;
    
    // Create temporary directory with same name as archive (without extension)
    const tempDir = `vrc-stt-rs-${version}`;
    if (!fs.existsSync(tempDir)) {
        fs.mkdirSync(tempDir);
    }

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
    console.log(`Creating archive: ${archiveName}`);
    try {
        execSync(`7z a "${archiveName}" "${tempDir}"/*`, {
            stdio: 'inherit'
        });
        console.log(`Archive created successfully: ${archiveName}`);
    } catch (error) {
        console.error('Error creating archive:', error.message);
        // Cleanup temp directory
        execSync(`rm -rf "${tempDir}"`);
        process.exit(1);
    }

    // Cleanup temp directory
    execSync(`rm -rf "${tempDir}"`);
    console.log('Temporary directory cleaned up.');
    console.log('Packaging process completed successfully!');
}

// Run the build and packaging
buildAndPackage();