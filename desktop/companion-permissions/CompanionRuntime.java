package stepx;

import android.os.IBinder;
import android.os.Process;
import android.system.Os;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;
import org.json.JSONObject;

/** Exactly one OEM freezer exemption; original membership is durable before any change. */
final class CompanionRuntime {
    private static final String PACKAGE="com.stepx.companion.app";
    private static final File DIRECTORY=new File("/data/system/stepx-companion");
    private static String dump(String operation)throws Exception{
        if(!Arrays.asList("whitelist","whiteAdd","whiteRemove").contains(operation))throw new SecurityException("Unsupported freezer operation");
        File temp=File.createTempFile("freeze-",".txt",DIRECTORY);
        try {
            Os.chmod(temp.getPath(),0600);
            IBinder manager=(IBinder)Class.forName("android.os.ServiceManager").getMethod("getService",String.class).invoke(null,"activity");
            try(FileOutputStream stream=new FileOutputStream(temp)){
                manager.dump(stream.getFD(),operation.equals("whitelist")?new String[]{"freeze",operation}:new String[]{"freeze",operation,PACKAGE});
                stream.getFD().sync();
            }
            if(temp.length()>131072)throw new IOException("Unexpected freezer output");
            return new String(Files.readAllBytes(temp.toPath()),StandardCharsets.UTF_8);
        }finally{temp.delete();}
    }
    private static List<String> membership(String value)throws Exception{
        List<String> sections=new ArrayList<>();String section="";boolean known=false;
        for(String raw:value.split("\\n")){
            String line=raw.trim();
            if(line.equals("cloud freeze white apps:")){section="cloud";known=true;}
            else if(line.equals("user freeze white apps:"))section="user";
            else if(line.equals("local freeze white apps:"))section="local";
            else if(line.equals("freeze white app: "+PACKAGE)){
                if(section.isEmpty()||sections.contains(section))throw new IOException("Ambiguous freezer membership");
                sections.add(section);
            }
        }
        if(!known)throw new IOException("Unsupported OEM freezer output");
        return sections;
    }
    static JSONObject restore(String signer,int uid)throws Exception{
        if(Process.myUid()!=1000)throw new SecurityException("System runtime required");
        String boot=new String(Files.readAllBytes(new File("/proc/sys/kernel/random/boot_id").toPath()),StandardCharsets.UTF_8).trim();
        if(!boot.matches("[0-9a-f-]{36}"))throw new IOException("Invalid boot identity");
        File journal=new File(DIRECTORY,"freezer-"+boot+".json");
        if(!journal.exists())return new JSONObject().put("restored",false).put("owned",false);
        JSONObject baseline=new JSONObject(new String(Files.readAllBytes(journal.toPath()),StandardCharsets.UTF_8));
        if(!signer.equals(baseline.getString("signer"))||baseline.getInt("uid")!=uid||!boot.equals(baseline.getString("boot_id")))throw new SecurityException("Runtime recovery identity mismatch");
        if(!baseline.getBoolean("added"))return new JSONObject().put("restored",false).put("owned",false);
        String current=dump("whitelist");List<String> present=membership(current);
        if(present.contains("user"))throw new SecurityException("Runtime exemption changed by user");
        if(!present.isEmpty()){
            dump("whiteRemove");String after=dump("whitelist");
            if(!membership(after).isEmpty())throw new IOException("Runtime rollback read-back failed");
            String targetLine="(?m)^\\s*freeze white app: com\\.stepx\\.companion\\.app\\s*$";
            if(!current.replaceAll(targetLine,"").replaceAll("\\s+"," ").trim().equals(after.replaceAll("\\s+"," ").trim()))throw new IOException("Other runtime lists changed during rollback");
        }
        return new JSONObject().put("restored",true).put("owned",true);
    }
    static JSONObject ensure(String signer,int uid)throws Exception{
        if(Process.myUid()!=1000)throw new SecurityException("System runtime required");
        if(!DIRECTORY.isDirectory()&&!DIRECTORY.mkdir())throw new IOException("Cannot create runtime recovery directory");
        Os.chmod(DIRECTORY.getPath(),0700);
        String boot=new String(Files.readAllBytes(new File("/proc/sys/kernel/random/boot_id").toPath()),StandardCharsets.UTF_8).trim();
        if(!boot.matches("[0-9a-f-]{36}"))throw new IOException("Invalid boot identity");
        File journal=new File(DIRECTORY,"freezer-"+boot+".json");
        String before=dump("whitelist");List<String> present=membership(before);
        JSONObject baseline;
        if(journal.exists()){
            baseline=new JSONObject(new String(Files.readAllBytes(journal.toPath()),StandardCharsets.UTF_8));
            if(!signer.equals(baseline.getString("signer"))||baseline.getInt("uid")!=uid||!boot.equals(baseline.getString("boot_id")))throw new SecurityException("Runtime recovery identity mismatch");
        }else{
            baseline=new JSONObject().put("signer",signer).put("uid",uid).put("boot_id",boot).put("before",before).put("added",present.isEmpty());
            File temporary=new File(DIRECTORY,journal.getName()+".new");
            if(!temporary.createNewFile())throw new IOException("Incomplete runtime recovery write");
            Os.chmod(temporary.getPath(),0600);
            try(FileOutputStream stream=new FileOutputStream(temporary)){stream.write(baseline.toString().getBytes(StandardCharsets.UTF_8));stream.getFD().sync();}
            Os.rename(temporary.getPath(),journal.getPath());
        }
        if(present.isEmpty())dump("whiteAdd");
        List<String> after=membership(dump("whitelist"));
        if(after.isEmpty()||(present.isEmpty()&&(!after.contains("cloud")||!after.contains("local"))))throw new IOException("Freezer exemption read-back failed");
        return new JSONObject().put("background_exempt",true).put("added",baseline.getBoolean("added")).put("boot_id",boot).put("journal",journal.getName());
    }
}
