#!/bin/bash

# Fix test_quantum_measurement
sed -i '' '651,656s/let states = vec\[/let mut states = vec\[/g' src/quantum_engine.rs
sed -i '' '656a\        normalize_states(&mut states);' src/quantum_engine.rs

# Fix test_wave_function_collapse  
sed -i '' '679,684s/let states = vec\[/let mut states = vec\[/g' src/quantum_engine.rs
sed -i '' '684a\        normalize_states(&mut states);' src/quantum_engine.rs

# Fix test_entanglement_correlation
sed -i '' '710,711s/let states1 = vec\[/let mut states1 = vec\[/g' src/quantum_engine.rs
sed -i '' '711,712s/let states2 = vec\[/let mut states2 = vec\[/g' src/quantum_engine.rs
sed -i '' '712a\        normalize_states(&mut states1);\
        normalize_states(&mut states2);' src/quantum_engine.rs

# Fix test_decoherence_time
sed -i '' '741,746s/let states = vec\[/let mut states = vec\[/g' src/quantum_engine.rs
sed -i '' '746a\        normalize_states(&mut states);' src/quantum_engine.rs

# Fix test_multiple_quantum_positions
sed -i '' '770,776s/states\.push(create_quantum_state/let mut state = create_quantum_state/g' src/quantum_engine.rs
sed -i '' '776s/));/); states.push(state);/g' src/quantum_engine.rs
sed -i '' '777a\            normalize_states(&mut states);' src/quantum_engine.rs

# Fix test_quantum_state_interference
sed -i '' '802,807s/let states = vec\[/let mut states = vec\[/g' src/quantum_engine.rs
sed -i '' '807a\        normalize_states(&mut states);' src/quantum_engine.rs

# Fix test_partial_measurement
sed -i '' '826,833s/let states = vec\[/let mut states = vec\[/g' src/quantum_engine.rs
sed -i '' '833a\        normalize_states(&mut states);' src/quantum_engine.rs

# Fix test_concurrent_access
sed -i '' '870,875s/let states = vec\[/let mut states = vec\[/g' src/quantum_engine.rs
sed -i '' '875a\                normalize_states(&mut states);' src/quantum_engine.rs

echo "Fixed quantum tests normalization issues"