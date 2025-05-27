require("@nomicfoundation/hardhat-toolbox");

/** @type import('hardhat/config').HardhatUserConfig */
module.exports = {
  solidity: "0.8.27",
  settings: {
    optimizer: {
      enabled: true,
      runs: 900,
    }
  }
};

// Import necessary modules
const fs = require('fs');
const path = require('path');
const { task } = require('hardhat/config');

// Extend the compile task
task('compile', 'Compiles the entire project, building all artifacts')
  .setAction(async (taskArgs, hre, runSuper) => {
    // Run the original compile task
    await runSuper(taskArgs);

    // Bytecode extraction logic
    console.log('\nExtracting bytecode');

    // Directory where the artifacts are stored
    const artifactsDir = path.join(__dirname, 'artifacts', 'contracts');

    // Directory to store the extracted bytecode
    const outputDir = path.join(__dirname, 'out');
    if (!fs.existsSync(outputDir)) {
      fs.mkdirSync(outputDir);
    }

    // Function to recursively process artifact directories
    const processArtifacts = (dir) => {
      const files = fs.readdirSync(dir);
      for (const file of files) {
        const fullPath = path.join(dir, file);
        const stat = fs.statSync(fullPath);

        if (stat.isDirectory()) {
          // Recursively process subdirectories
          processArtifacts(fullPath);
        } else if (file.endsWith('.json')) {
          // Process artifact JSON files
          const artifact = JSON.parse(fs.readFileSync(fullPath, 'utf8'));
          const bytecode = artifact.bytecode;
          const deployedBytecode = artifact.deployedBytecode;
          let abi = artifact.abi;
          const contractName = artifact.contractName || path.basename(file, '.json');

          if (bytecode && contractName) {
            const outputPath = path.join(outputDir, `${contractName}.txt`);
            fs.writeFileSync(outputPath, bytecode);
            console.log(`- Bytecode for ${contractName} extracted to ${outputPath}`);

            const outputPathDep = path.join(outputDir, `${contractName}.dep.txt`);
            fs.writeFileSync(outputPathDep, deployedBytecode);
            console.log(`- Deployed bytecode for ${contractName} extracted to ${outputPathDep}`);

            const outputPathAbi = path.join(outputDir, `${contractName}.abi.json`);
            fs.writeFileSync(outputPathAbi, JSON.stringify(abi, null, 4));
            console.log(`- ABI for ${contractName} extracted to ${outputPathAbi}`);
          }
        }
      }
    };

    // Start processing artifacts
    processArtifacts(artifactsDir);

    console.log('Bytecode extraction complete.\n');
  });
