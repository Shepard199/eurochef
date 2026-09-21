import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;

public class AiHitReactionVtableCensus extends GhidraScript {
    @Override public void run() throws Exception {
        String[] names = new String[] {
            "MonsterBase", "Monster2Rockets", "ConstructionBot", "DogBot", "Eb07MineBot",
            "Eb10RollerBot", "Eb11MagnaBot", "Eb12EvilBot", "Eb13KnightBot", "Eb14Minion",
            "Eb15Launcher", "Eb16KnuckleBot", "Ef01Mine", "Ef03EvilBot", "Em07PiranhaBot",
            "Ep02Turret", "Ep04Turret", "Ep05Turret", "Ep06Turret", "Eq02MineBot",
            "Eq03Spider", "Eq04Mine", "Ew07Dodgem", "Ew08Flambe", "Ew08FlambeLarge",
            "Ew09Armoured", "Ew10Minion", "Ew11FatBot", "GuardBot", "JailBotLarge",
            "JailBotNormal", "MalfBot", "SawBot", "SecurityBot", "ShieldBot", "ShuntBot",
            "ShuntBotBoss", "SpikeBot", "SpinTop", "Sweeper", "TestAnimBot", "ThiefBot",
            "TurretBot", "Npc", "NpcFender"
        };
        long[] vtables = new long[] {
            0x005E2920L, 0x005E2D58L, 0x005E3A00L, 0x005E2BF0L, 0x005E3FA0L,
            0x005E5520L, 0x005E53B8L, 0x005E5F00L, 0x005E6338L, 0x005E61D0L,
            0x005E64A0L, 0x005E6608L, 0x005E5AC8L, 0x005E2920L, 0x005E2920L,
            0x005E4828L, 0x005E49A0L, 0x005E4B18L, 0x005E4C90L, 0x005E43E0L,
            0x005E5D98L, 0x005E5960L, 0x005E4F70L, 0x005E5690L, 0x005E57F8L,
            0x005E50E0L, 0x005E6068L, 0x005E2920L, 0x005E3B68L, 0x005E3898L,
            0x005E3730L, 0x005E4E08L, 0x005E3028L, 0x005E3E38L, 0x005E3CD0L,
            0x005E3190L, 0x005E32F8L, 0x005E2EC0L, 0x005E4270L, 0x005E4548L,
            0x005E2920L, 0x005E4108L, 0x005E3460L, 0x005E7048L, 0x005E71B8L
        };
        Memory mem = currentProgram.getMemory();
        println("name,vtable,slot_c8");
        for (int i = 0; i < names.length; i++) {
            Address slot = toAddr(vtables[i] + 0xC8L);
            long target = Integer.toUnsignedLong(mem.getInt(slot));
            println(String.format("%s,0x%08X,0x%08X", names[i], vtables[i], target));
        }
    }
}
