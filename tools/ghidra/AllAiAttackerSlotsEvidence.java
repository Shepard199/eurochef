import ghidra.app.script.GhidraScript;
import java.util.*;
public class AllAiAttackerSlotsEvidence extends GhidraScript {
 record R(String n,long v){}
 long p(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
 @Override public void run()throws Exception{
  R[] r={
   new R("MonsterBase",0x005E2920L),new R("Monster2Rockets",0x005E2D58L),new R("ConstructionBot",0x005E3A00L),
   new R("DogBot",0x005E2BF0L),new R("Eb07MineBot",0x005E3FA0L),new R("Eb10RollerBot",0x005E5520L),
   new R("Eb11MagnaBot",0x005E53B8L),new R("Eb12EvilBot",0x005E5F00L),new R("Eb13KnightBot",0x005E6338L),
   new R("Eb14Minion",0x005E61D0L),new R("Eb15Launcher",0x005E64A0L),new R("Eb16KnuckleBot",0x005E6608L),
   new R("Ef01Mine",0x005E5AC8L),new R("Ef03EvilBot",0x005E68D8L),new R("Em07PiranhaBot",0x005E6A40L),
   new R("Ep02Turret",0x005E4828L),new R("Ep04Turret",0x005E49A0L),new R("Ep05Turret",0x005E4B18L),new R("Ep06Turret",0x005E4C90L),
   new R("Eq02MineBot",0x005E43E0L),new R("Eq03Spider",0x005E5D98L),new R("Eq04Mine",0x005E5960L),
   new R("Ew07Dodgem",0x005E4F70L),new R("Ew08Flambe",0x005E5690L),new R("Ew08FlambeLarge",0x005E57F8L),new R("Ew09Armoured",0x005E50E0L),
   new R("Ew10Minion",0x005E6068L),new R("Ew11FatBot",0x005E6770L),new R("GuardBot",0x005E3B68L),
   new R("JailBotLarge",0x005E3898L),new R("JailBotNormal",0x005E3730L),new R("MalfBot",0x005E4E08L),new R("SawBot",0x005E3028L),
   new R("SecurityBot",0x005E3E38L),new R("ShieldBot",0x005E3CD0L),new R("ShuntBot",0x005E3190L),new R("ShuntBotBoss",0x005E32F8L),
   new R("SpikeBot",0x005E2EC0L),new R("SpinTop",0x005E4270L),new R("Sweeper",0x005E4548L),new R("TestAnimBot",0x005E6BA8L),
   new R("ThiefBot",0x005E4108L),new R("TurretBot",0x005E3460L),new R("Npc",0x005E7048L),new R("NpcFender",0x005E71B8L)
  };
  Map<String,List<String>> groups=new LinkedHashMap<>();
  for(R x:r){
   long a=p(x.v()+0x14c),b=p(x.v()+0x15c);
   String k=String.format("14C=0x%08X 15C=0x%08X",a,b);
   groups.computeIfAbsent(k,z->new ArrayList<>()).add(x.n());
  }
  for(var e:groups.entrySet())println(e.getKey()+" :: "+String.join(",",e.getValue()));
 }
}