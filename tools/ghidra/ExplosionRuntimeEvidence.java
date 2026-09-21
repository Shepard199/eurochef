import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;

/**
 * Repeatable evidence pass for the common XItemHandler_Explosion runtime.
 *
 * Keeps the UE-facing reverse frontier in one place instead of re-proving the
 * same constructor/scheduler/fragment/projectile/HitQuery relationships by hand.
 */
public class ExplosionRuntimeEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface decompiler) throws Exception {
        Address address = toAddr(raw);
        println(String.format("\n=== 0x%08X ===", raw));
        Function function = getFunctionContaining(address);
        if (function == null) {
            disassemble(address);
            function = createFunction(address, null);
        }
        if (function == null) {
            println("NO_FUNCTION");
            return;
        }
        println("FUNCTION=" + function.getName() + " ENTRY=" + function.getEntryPoint());
        DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
        if (result.decompileCompleted() && result.getDecompiledFunction() != null) {
            println(result.getDecompiledFunction().getC());
        }
    }

    private void dumpVtable(Memory memory, long vtable, long[] slots, String label)
            throws Exception {
        println(String.format("\n=== %s VTABLE 0x%08X ===", label, vtable));
        for (long slot : slots) {
            long target = memory.getInt(toAddr(vtable + slot)) & 0xffffffffL;
            println(String.format("%s +0x%02X -> 0x%08X", label, slot, target));
        }
    }

    private void dumpFloat(Memory memory, long address, String label) throws Exception {
        int bits = memory.getInt(toAddr(address));
        println(String.format(
                "%s 0x%08X bits=0x%08X value=%s",
                label,
                address,
                bits,
                Float.toString(Float.intBitsToFloat(bits))));
    }

    @Override
    public void run() throws Exception {
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        Memory memory = currentProgram.getMemory();

        // XItemHandler_Explosion. +0x34 is the periodic scheduler recovered at
        // 0x004DCBA0; +0x04 is the destructor path through 0x004DC500.
        dumpVtable(memory, 0x005F1470L, new long[]{0x00L, 0x04L, 0x34L, 0x38L},
                "XItemHandler_Explosion");

        // Inner object allocated at 0x004DD0EF (size 0x220), proving that
        // 0x0041EAC0 belongs to XItemPhysics_Projectile rather than Explosion.
        dumpVtable(memory, 0x005DFE18L, new long[]{0x00L, 0x5CL},
                "XItemPhysics_Projectile");

        for (long address : new long[]{
                0x004DC510L, // explosion definition lookup + main XItem creation
                0x004DC6A0L, // XItemHandler_Explosion initialization
                0x004DCBA0L, // periodic fragment/HitQuery service
                0x004DCC20L, // ten-slot fragment sequence factory
                0x004DCCE0L, // fragment child XItem + projectile physics setup
                0x004DB6B0L, // XExplosionFragment main Handler update / lifetime
                0x004DB8B0L, // XExplosionFragment Physics/contact response override
                0x004DBB20L, // XExplosionFragment teardown
                0x004DBCC0L, // XExplosionFragment visibility fade helper
                0x004DBEF0L, // staggered Player pickup-attraction/latch service
                0x004DBF90L, // staggered Player pickup-collection service
                0x004DA8B0L, // pickup UID -> Player inventory bitset classification
                0x004AFF20L, // Player pickup-attraction wrapper / default range + offset bridge
                0x004DB190L, // replace/initialize XItemPhysics_PickupAttract on the fragment
                0x004DA910L, // standalone XItemPhysics_PickupAttract factory/ctor
                0x004DA9D0L, // XItemPhysics_PickupAttract fixed-step spring/damping update
                0x004DB330L, // current target+offset-fragment displacement getter
                0x004B0D50L, // Player pickup-attraction default radius
                0x004B0D60L, // Player pickup-attraction target offset
                0x0040AA30L, // Player inventory container bitset classifier
                0x0040AA70L, // Player inventory non-mutating add preflight
                0x0040ABA0L, // Player inventory counter commit
                0x0040AE90L, // Player inventory bitset commit
                0x004DC0F0L, // staggered vertical teardown / pickup-position service
                0x004DC1B0L, // dynamic pickup/resource factory bridge
                0x004DC2E0L, // fragment Physics + embedded HitQuery initialization
                0x00425800L, // explicit-shape HitQuery service used by moving fragment
                0x0041A2D0L, // Physics embedded sphere init (center 0, radius argument)
                0x004D64C0L, // native hit-shape materialization
                0x004D6560L, // native hit-shape radius/extent getter
                0x0041EAC0L, // XItemPhysics_Projectile ctor
                0x0041EB10L, // XItemPhysics_Projectile fixed update
                0x0041EC80L, // shared ballistic launch solver
                0x00425A70L, // common HitQuery initializer
                0x00425C70L, // candidate traversal / serial commit
                0x00403DB0L, // owner +0x98 generic XItem factory; registers priority 0x14
                0x004E9A1CL, // XItem manager sorted registration
                0x004E8188L, // XItem Handler-then-animator update ordering
                0x004FA5A8L, // EXItemAnimator_Script one-frame update
                0x004F97B2L, // active Script controller application
                0x004F938BL, // controller transform -> child animator
                0x00567861L, // EXItemAnimator_Collision datum provider
                0x005678FAL, // Collision runtime scale -> datum shape lanes
                0x004DA680L, // pickup hash -> native sub-id
                0x004DA710L  // pickup visual/resource variant selector
        }) {
            dump(address, decompiler);
        }

        // Constants consumed directly by 0x004DCCE0 / 0x0041EC80.
        dumpFloat(memory, 0x005DFFF4L, "fragment no-datum Y offset");
        dumpFloat(memory, 0x005E2338L, "fragment random-scale range");
        dumpFloat(memory, 0x005DF8D0L, "fragment random-scale minimum / pickup collect distance squared");
        dumpFloat(memory, 0x005DD938L, "Player pickup-attraction default range");
        dumpFloat(memory, 0x005DDEA4L, "PickupAttract ordinary force threshold squared");
        dumpFloat(memory, 0x005F1350L, "PickupAttract mode2/3/4 force threshold squared");
        dumpFloat(memory, 0x005DD3C4L, "PickupAttract normalization epsilon");
        dumpFloat(memory, 0x005DD93CL, "projectile / PickupAttract fixed step");

        decompiler.dispose();
    }
}
